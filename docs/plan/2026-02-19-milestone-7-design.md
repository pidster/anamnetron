# Milestone 7: TypeScript Analyzer — Design

## Goal

Add a TypeScript/Svelte analyzer to `crates/analyzer/`, proving the multi-language architecture works. Dog-food on this project's `web/` Svelte/TypeScript frontend.

## Scope

- TypeScript project discovery (via `package.json`)
- tree-sitter-typescript analysis (exported classes, functions, interfaces, types)
- Svelte script block extraction (lightweight parser, not a grammar crate)
- Import statement → Depends edge extraction
- Integration into existing `analyze_project()` orchestrator
- Dog-food: `svt analyze --project .` discovers both Rust crates and TypeScript packages

## Architecture

Extends the existing three-stage pipeline with a second language:

```
package.json  -> TS Discovery  -> TsPackageLayout
                                       |
.ts/.tsx/.svelte -> TypeScriptAnalyzer -> Vec<AnalysisItem>  (qualified names, raw relationships)
                                       |
                    Mapping (reused)   -> Vec<Node> + Vec<Edge>  (canonical paths, provenance)
                                       |
                    GraphStore         -> Analysis snapshot (same version as Rust analysis)
```

Both Rust and TypeScript analysis feed into the same snapshot. The existing `LanguageAnalyzer` trait and `map_to_graph()` pipeline are reused without modification.

## Module Structure

New and modified files in `crates/analyzer/`:

```
crates/analyzer/
  src/
    lib.rs                  -- MODIFY: add TS discovery + analysis phase
    discovery.rs            -- MODIFY: extract shared types, add discover_ts_packages()
    languages/
      mod.rs                -- EXISTING: LanguageAnalyzer trait (unchanged)
      rust.rs               -- EXISTING: unchanged
      typescript.rs          -- NEW: tree-sitter-typescript analysis
      svelte.rs             -- NEW: Svelte script block extraction
    mapping.rs              -- EXISTING: unchanged (:: separator already universal)
```

## TypeScript Discovery

### Types

```rust
pub struct TsPackageInfo {
    pub name: String,
    pub root: PathBuf,
    pub source_root: PathBuf,    // typically root/src/
    pub source_files: Vec<PathBuf>,  // .ts, .tsx, .svelte files
}
```

### How It Works

1. Walk `project_root` looking for `package.json` files.
2. Skip `node_modules/`, `dist/`, `build/`, `.svt/`, `target/` directories.
3. For each `package.json`: read `name` field (fallback: directory name).
4. Determine source root: `src/` if it exists, otherwise the package root.
5. Walk source root collecting `.ts`, `.tsx`, `.svelte` files.
6. Skip test files (`*.test.ts`, `*.spec.ts`) and declaration files (`*.d.ts`).

### Package Name Handling

The `name` field in `package.json` may be scoped (e.g. `@scope/name`). Strip the scope prefix — only the bare name is used for the qualified name root segment.

## Svelte Script Block Extraction

### Module: `languages/svelte.rs`

A lightweight parser (not tree-sitter) that extracts `<script>` block content from `.svelte` files.

```rust
pub struct ScriptBlock {
    pub content: String,
    pub line_offset: usize,   // line number of <script> tag in the .svelte file
    pub is_module: bool,      // true for <script context="module">
}

pub fn extract_script_blocks(source: &str) -> Vec<ScriptBlock>;
```

### How It Works

1. Scan for `<script` tags (with optional `lang="ts"` or `context="module"` attributes).
2. Find the closing `>` of the opening tag.
3. Find the matching `</script>` closing tag.
4. Extract the content between them.
5. Record the line offset so `source_ref` line numbers are correct relative to the original `.svelte` file.

### Svelte Component as Node

Each `.svelte` file produces a file-level node with NodeKind::Unit, sub_kind "component". The exported symbols within the script block are children of this component node.

## TypeScript Language Analysis

### Extraction Table

| TypeScript construct | NodeKind | sub_kind | Detection |
|---|---|---|---|
| Package (from package.json) | Service | `package` | Discovery, not tree-sitter |
| Directory (src/components/) | Component | `module` | File walking |
| .ts/.tsx file (as module) | Component | `module` | File walking |
| .svelte file | Unit | `component` | File walking + script extraction |
| `export class Foo` | Unit | `class` | `export_statement` > `class_declaration` |
| `export function foo()` | Unit | `function` | `export_statement` > `function_declaration` |
| `export interface Foo` | Unit | `interface` | `export_statement` > `interface_declaration` |
| `export type Foo = ...` | Unit | `type-alias` | `export_statement` > `type_alias_declaration` |
| `export default class` | Unit | `class` | `export_statement` with `default` |
| `export default function` | Unit | `function` | `export_statement` with `default` |

### Edge Extraction

| Source | EdgeKind | Detection |
|---|---|---|
| `import { X } from './lib/api'` | Depends | `import_statement` node |
| `import X from './lib/api'` | Depends | `import_statement` with default import |
| `export class X extends Y` | Extends | `class_heritage` with `extends` clause |
| `export class X implements Y` | Implements | `class_heritage` with `implements` clause |

### What Is NOT Extracted

- Private/non-exported declarations (per "module + exports" depth decision)
- Function call sites (too noisy without type information)
- Re-exports (`export { X } from './other'`) — treated as a Depends edge on the module
- Dynamic imports (`import()`) — not statically resolvable

## Qualified Name Convention

TypeScript items use `::` as separator, matching the Rust convention. This allows the existing `qualified_name_to_canonical()` function to work unchanged.

| TypeScript source | Qualified name | Canonical path |
|---|---|---|
| Package `svt-web` | `svt-web` | `/svt-web` |
| `src/components/` directory | `svt-web::components` | `/svt-web/components` |
| `src/lib/api.ts` file | `svt-web::lib::api` | `/svt-web/lib/api` |
| `GraphView.svelte` component | `svt-web::components::GraphView` | `/svt-web/components/graph-view` |
| `export function fetchNodes()` in `api.ts` | `svt-web::lib::api::fetchNodes` | `/svt-web/lib/api/fetch-nodes` |
| `export interface ApiNode` in `types.ts` | `svt-web::lib::types::ApiNode` | `/svt-web/lib/types/api-node` |

### File-to-Module Mapping

- File path relative to source root, with extension stripped
- Directory separators become `::` segments
- `index.ts` files collapse to their parent directory name (not `::index`)
- PascalCase file names (common in component files) are preserved in qualified name, then kebab-cased by the mapping module

## Integration into Orchestrator

Modify `crates/analyzer/src/lib.rs::analyze_project()`:

```rust
// Phase 1: Rust analysis (existing)
let rust_layout = discover_workspace(project_root)?;
// ... existing Rust analysis code ...

// Phase 2: TypeScript analysis (new)
let ts_packages = discover_ts_packages(project_root)?;
let ts_analyzer = TypeScriptAnalyzer::new();
for package in &ts_packages {
    let file_refs: Vec<&Path> = package.source_files.iter().map(|p| p.as_path()).collect();
    let result = ts_analyzer.analyze_crate(&package.name, &file_refs);
    all_items.extend(result.items);
    all_relations.extend(result.relations);
    all_warnings.extend(result.warnings);
}

// Phase 3: Mapping (existing, unchanged)
let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
```

### AnalysisSummary Changes

Add `ts_packages_analyzed` and update `files_analyzed` to include TypeScript/Svelte files.

## Dependencies

| Crate | Version | Justification |
|---|---|---|
| `tree-sitter-typescript` | 0.23 | TypeScript/TSX grammar, compatible with existing tree-sitter 0.24 |

No Svelte grammar crate needed — script block extraction is a lightweight custom parser.

## Dog-Food: Expected Output

After `svt analyze --project .`, the analysis snapshot should contain TypeScript nodes from `web/`:

| Expected canonical path | Kind | sub_kind |
|---|---|---|
| `/svt-web` | Service | package |
| `/svt-web/components` | Component | module |
| `/svt-web/components/graph-view` | Unit | component |
| `/svt-web/components/node-detail` | Unit | component |
| `/svt-web/components/conformance-report` | Unit | component |
| `/svt-web/components/search-bar` | Unit | component |
| `/svt-web/components/snapshot-selector` | Unit | component |
| `/svt-web/lib` | Component | module |
| `/svt-web/lib/api` | Component | module |
| `/svt-web/lib/types` | Component | module |
| `/svt-web/stores` | Component | module |
| `/svt-web/stores/graph` | Component | module |
| `/svt-web/stores/selection` | Component | module |

Plus exported functions/interfaces/types from each module.

## Testing Strategy

### Unit tests (languages/typescript.rs)
- Parse `.ts` with exported function → AnalysisItem with correct qualified name
- Parse `.ts` with exported class, interface, type alias → correct kinds/sub_kinds
- Parse import statements → AnalysisRelation with Depends kind
- `export default function` → correct item extraction
- Empty file → no items
- File with only private declarations → no items
- Nested directory structure → correct parent qualified names

### Unit tests (languages/svelte.rs)
- `<script lang="ts">` block extracted with correct content
- `<script>` block (no lang attribute) extracted
- Line offset calculation is correct
- No script block → empty result
- Multiple script blocks (module + instance) → both extracted
- Malformed script tags handled gracefully

### Integration tests
- Discover a TypeScript package from fixture directory
- Analyze produces correct node and edge counts
- Mixed Rust + TypeScript project produces nodes for both languages

### Dog-food tests
- `analyze_project()` on workspace root finds both Rust and TypeScript
- Analysis snapshot contains TypeScript nodes
- Conformance check passes (design already has `/svt/web` nodes)

### Snapshot tests (insta)
- Extracted items from a fixture `.ts` file for regression detection
