# Milestone 3: Rust Analyzer, Conformance Comparison, CLI — Design

## Goal

Full discovery-mode pipeline: analyze a Rust project with tree-sitter, produce an analysis snapshot, compare it against a design snapshot, and report drift. Dog-food on this project itself.

## Scope

- Rust project discovery (via `cargo_metadata`)
- Full-depth tree-sitter analysis (crates, modules, types, functions, call edges)
- Canonical path mapping (qualified names to kebab-case paths)
- Conformance comparison (design vs analysis: unimplemented, undocumented, constraint evaluation)
- CLI command: `svt analyze`
- Updated `svt check --analysis`
- Dog-food: analyze this project, compare against `design/architecture.yaml`

## Architecture: Layered Pipeline

Three-stage pipeline: Discovery -> Parse -> Map.

```
Cargo.toml -> Discovery -> ProjectLayout
                               |
.rs files  -> RustAnalyzer -> Vec<AnalysisItem>  (qualified names, raw relationships)
                               |
              Mapping      -> Vec<Node> + Vec<Edge>  (canonical paths, provenance)
                               |
              GraphStore   -> Analysis snapshot (version N)
```

All new modules in `crates/analyzer/`. The analyzer depends on `svt-core` only. Core has no dependency on analyzer (enforced by existing constraints).

## Module Structure

```
crates/analyzer/
  src/
    lib.rs              -- public API: analyze_project()
    discovery.rs        -- cargo_metadata adapter, file walking
    languages/
      mod.rs            -- LanguageAnalyzer trait
      rust.rs           -- tree-sitter-rust: crates, modules, types, functions, calls
    mapping.rs          -- AnalysisItem -> Node/Edge with canonical paths
    types.rs            -- intermediate types: ProjectLayout, AnalysisItem, AnalysisResult
```

### Public API

```rust
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary>;
```

Creates an `Analysis` snapshot, populates it, returns a summary.

## Discovery Module

Thin adapter over `cargo_metadata` plus a file walker for `.rs` sources.

### Types

```rust
pub enum CrateType {
    Lib,
    Bin,
}

pub struct CrateInfo {
    pub name: String,
    pub crate_type: CrateType,
    pub root: PathBuf,
    pub entry_point: PathBuf,
    pub source_files: Vec<PathBuf>,
}

pub struct ProjectLayout {
    pub workspace_root: PathBuf,
    pub crates: Vec<CrateInfo>,
}
```

A crate with both `lib.rs` and `main.rs` emits two `CrateInfo` entries (one `Lib`, one `Bin`) sharing the same `root` but different entry points.

### How It Works

1. Run `cargo_metadata::MetadataCommand` at `project_root`.
2. Map `workspace_packages()` to `CrateInfo` structs. Each target with kind `lib` or `bin` becomes a `CrateInfo`.
3. Walk each crate's `src/` directory to collect all `.rs` files.
4. Only workspace-local crates. External dependencies are not analyzed.

## Language Analysis: Rust tree-sitter Queries

### Intermediate Types

```rust
pub struct AnalysisItem {
    pub qualified_name: String,
    pub kind: NodeKind,
    pub sub_kind: String,
    pub parent_qualified_name: Option<String>,
    pub source_ref: String,
    pub language: String,
}

pub struct AnalysisRelation {
    pub source_qualified_name: String,
    pub target_qualified_name: String,
    pub kind: EdgeKind,
}

pub struct AnalysisWarning {
    pub source_ref: String,
    pub message: String,
}
```

### Extraction Table

| Rust construct | NodeKind | sub_kind | Detection |
|---|---|---|---|
| Crate (from CrateInfo) | Service | `crate` | From discovery, not tree-sitter |
| `mod foo` / `mod foo { }` | Component | `module` | `mod_item` node |
| `pub struct Foo` | Unit | `struct` | `struct_item` node |
| `pub enum Foo` | Unit | `enum` | `enum_item` node |
| `pub trait Foo` | Unit | `trait` | `trait_item` node |
| `impl Trait for Type` | -- | -- | Generates `Implements` edge |
| `pub fn foo()` | Unit | `function` | `function_item` node |
| `use crate::foo::Bar` | -- | -- | Generates `Depends` edge |
| `foo::bar()` call sites | -- | -- | Generates `Calls` edge |

### Containment

- Crate contains modules (from `mod` declarations and file structure)
- Modules contain types and functions
- Generated as `Contains` edges

### Edge Extraction

- `use` statements -> `Depends` edges (module-to-module or item-to-item)
- `impl Trait for Type` -> `Implements` edge
- Function call expressions -> `Calls` edges (best-effort; tree-sitter is syntactic)

### Unresolvable Calls

Tree-sitter is syntactic, not semantic. Some calls cannot be resolved to a qualified name. These are logged as `AnalysisWarning` rather than silently dropped. Warnings are collected in the `AnalysisSummary` and stored in snapshot metadata.

## Mapping Module

Pure function transforming qualified names to canonical paths.

### Rust Mapping Rules

| Qualified name | Canonical path |
|---|---|
| `svt_core` | `/svt-core` |
| `svt_core::model` | `/svt-core/model` |
| `svt_core::model::Node` | `/svt-core/model/node` |
| `svt_core::store::CozoStore` | `/svt-core/store/cozo-store` |

### Transformation Steps

1. Split on `::` to get segments
2. Apply `to_kebab_case()` to each segment (exists in `canonical.rs`)
3. Join with `/`, prepend `/`
4. Validate with `validate_canonical_path()`

### Edge Mapping

Replace qualified names in source/target with canonical paths. Drop edges where either endpoint didn't resolve (log a warning).

### ID Generation

Deterministic IDs from canonical path + version so re-analysis of the same code produces stable IDs.

### Function Signature

```rust
pub fn map_to_graph(
    items: &[AnalysisItem],
    relations: &[AnalysisRelation],
) -> (Vec<Node>, Vec<Edge>, Vec<AnalysisWarning>)
```

Pure function -- no store access, no I/O. Table-driven testable.

## Conformance Comparison

Implements the real `evaluate()` function (currently a stub).

### Three Outputs

1. **Unimplemented nodes** -- in design but not in analysis
2. **Undocumented nodes** -- in analysis but not in design
3. **Constraint evaluation** -- constraints run against analysis edges

### Matching

Nodes matched by canonical path. No fuzzy matching.

### Depth Tolerance

The design model is typically coarser than analysis. A design node is "implemented" if its canonical path exists in analysis OR if any analysis node is a descendant of it. This prevents design `Component` nodes from showing as "unimplemented" when the analysis went deeper.

### Undocumented Filtering

Only report undocumented nodes at the same depth as design nodes. If the design goes to Component depth, only flag undocumented Components, not every Unit.

### Constraint Evaluation on Analysis

Run existing `must_not_depend` evaluator against the analysis version's edges to check real code dependencies against design constraints.

### Report

```rust
pub struct ConformanceReport {
    pub design_version: Version,
    pub analysis_version: Option<Version>,  // now populated
    pub constraint_results: Vec<ConstraintResult>,
    pub unimplemented: Vec<UnmatchedNode>,  // now populated
    pub undocumented: Vec<UnmatchedNode>,   // now populated
    pub summary: ConformanceSummary,        // counts now populated
}
```

## CLI Commands

### `svt analyze [path]`

```
svt analyze [path]           # Analyze a Rust project (default: current dir)
    --store <path>           # Store location (default: .svt/store)
    --commit-ref <ref>       # Optional git commit ref to tag the snapshot
```

Flow: open/create store -> call `analyze_project` -> print summary, warnings to stderr -> exit 0.

### Updated `svt check`

```
svt check
    --analysis <version>     # Compare design against analysis version (NEW)
    --store <path>
    --design <version>
    --fail-on <severity>
    --format <format>
```

When `--analysis` is provided, calls `evaluate()` instead of `evaluate_design()`. Human-readable output gains sections for unimplemented/undocumented nodes.

## Dependencies

| Crate | Where | Justification |
|---|---|---|
| `cargo_metadata` | svt-analyzer | Workspace/crate discovery, avoids reimplementing Cargo logic |
| `tree-sitter` (0.24) | svt-analyzer | Already present. Language-agnostic parsing framework |
| `tree-sitter-rust` | svt-analyzer | Rust grammar for tree-sitter |
| `walkdir` | svt-analyzer | Recursive `.rs` file discovery |

## Testing Strategy

### Discovery tests
- Workspace with multiple crates produces correct `ProjectLayout`
- Single-crate project (no workspace) works
- Crate with both lib and bin emits two `CrateInfo` entries
- Non-Rust project gives clear error

### Parser tests (tree-sitter)
- Module extraction from file structure
- Struct/enum/trait extraction with correct qualified names
- Function extraction
- `use` statement -> Depends edge
- `impl Trait for Type` -> Implements edge
- Call site extraction and warning on unresolvable
- Nested modules (inline and file-based)

### Mapping tests
- Qualified name -> canonical path for all Rust patterns
- `to_kebab_case` integration (PascalCase structs, snake_case modules)
- Unresolvable edge endpoints produce warnings
- ID generation is deterministic

### Conformance comparison tests
- Design node with matching analysis node -> not unimplemented
- Design node with descendant analysis nodes -> not unimplemented (depth tolerance)
- Analysis node with no design match -> undocumented (at correct depth)
- Constraints evaluated against analysis edges
- Full pipeline: load design YAML, analyze Rust project, compare

### Dog-food tests
- Analyze this project, compare against `design/architecture.yaml`
- All `must_not_depend` constraints should pass on real code
- Core modules should appear in analysis
- No unexpected unimplemented design nodes

## Engineering Process: Concentric Validation Loops

Implementation uses an agent team with four validation levels:

| Level | Agent | Validates | Frequency |
|---|---|---|---|
| Test | test-validator | TDD compliance, coverage, edge cases | After every task |
| Code | code-validator | Clippy, conventions, error types, docs | After every task |
| Design | design-validator | Alignment with this design doc, architecture.yaml | After each module |
| Theory | theory-validator | Conceptual soundness of abstractions, mapping rules, edge cases | At 3 integration points |

Two builder agents implement tasks in parallel. Inner loops (test + code) run after every task. Outer loops (design + theory) run at module boundaries and integration points.
