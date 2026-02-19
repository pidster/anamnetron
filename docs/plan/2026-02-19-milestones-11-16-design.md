# Milestones 11–16 Design

## Context

Milestones 1–10 established the core product: data model, graph store, interchange format, conformance engine, Rust + TypeScript analyzers, web UI, WASM bridge, CI pipeline, and plugin foundations (registries + traits). This document scopes the remaining work.

---

## Milestone 11: Analyzer Registry Wiring + Canonical Path Alignment

**Priority:** High — this is the most impactful next step. It closes the gap between "all constraints pass in design-only mode" and "all constraints pass in full conformance mode."

### Problem 1: Canonical Path Misalignment

The analyzer's `qualified_name_to_canonical()` in `crates/analyzer/src/mapping.rs` converts Rust qualified names to canonical paths by splitting on `::` and kebab-casing each segment. This produces:

| Rust qualified name | Analyzer canonical path | Design canonical path |
|---|---|---|
| `svt_core` | `/svt-core` | `/svt/core` |
| `svt_core::model` | `/svt-core/model` | `/svt/core/model` |
| `svt_cli` | `/svt-cli` | `/svt/cli` |
| `svt_analyzer` | `/svt-analyzer` | `/svt/analyzer` |

The root issue: Rust crate names use `svt_core` (one underscore-separated token) but the design model uses `/svt` as the workspace root with crates as children (`/svt/core`, `/svt/cli`).

**Solution: Workspace-aware canonical path mapping.**

The analyzer should detect the workspace name (from `Cargo.toml [workspace]` or the common crate name prefix) and use it as the root namespace. A crate named `svt_core` in workspace `svt` becomes `/svt/core`, not `/svt-core`. Similarly, `svt-server` becomes `/svt/server`.

Specifically:
1. Add a `workspace_prefix` field to the project layout discovery (`discover_project()`), derived from the workspace root name or `Cargo.toml` metadata.
2. In `qualified_name_to_canonical()`, when the first segment matches the workspace prefix, split it: `svt_core` → segments `["svt", "core"]` instead of `["svt-core"]`.
3. This is backwards-compatible — projects without a detectable workspace prefix continue to use the current behaviour.

### Problem 2: Missing Structural Nodes

The 4 not-evaluable dog-food constraints are:

| Constraint | Kind | Scope | Issue |
|---|---|---|---|
| `cli-has-check-command` | `must_contain` | `/svt/cli/commands` | Analyzer produces no node at `/svt/cli/commands` — CLI functions (`run_check`, `run_export`) are in `main.rs`, not a `commands` module |
| `cli-has-analyze-command` | `must_contain` | `/svt/cli/commands` | Same |
| `cli-has-export-command` | `must_contain` | `/svt/cli/commands` | Same |
| `core-model-fan-in` | `max_fan_in` | `/svt/core/model` | Path misalignment — node exists at `/svt-core/model` not `/svt/core/model` |

The `max_fan_in` constraint is fixed by solving Problem 1 (path alignment).

The `must_contain` constraints require the CLI analyzer to either:
- (a) Detect the `clap` command structure from the source code and emit command nodes, or
- (b) Allow the design model to specify path aliases or looser matching for conformance

**Recommended approach:** (a) — The Rust analyzer should detect `#[derive(Parser)]` / `#[derive(Subcommand)]` and emit `Unit` nodes for each subcommand variant. This gives the conformance engine real structural nodes to match against.

### Problem 3: AnalyzerRegistry Not Wired

`analyze_project()` in `crates/analyzer/src/lib.rs` hardcodes two analysis phases:

```rust
// Phase 1: Rust analysis
let rust_analyzer = RustAnalyzer::new();
// Phase 2: TypeScript/Svelte analysis
let ts_analyzer = TypeScriptAnalyzer::new();
```

The `AnalyzerRegistry` exists but is never used in the main pipeline. The `LanguageAnalyzer` trait only has `analyze_crate()` — it lacks discovery methods.

**Solution:** Extend the trait and refactor the orchestrator:

1. Add to `LanguageAnalyzer`:
   - `fn can_analyze(&self, path: &Path) -> bool` — quick check if a file belongs to this language
   - `fn discover(&self, root: &Path) -> Result<Vec<ProjectUnit>>` — discover compilable units (crates, packages) in a project

2. Refactor `analyze_project()` to:
   ```
   for analyzer in registry.all() {
       let units = analyzer.discover(root)?;
       for unit in units {
           let result = analyzer.analyze_crate(&unit.name, &unit.source_files);
           // ... map and insert
       }
   }
   ```

3. Move Rust-specific crate discovery from `discover_project()` into `RustAnalyzer::discover()`.
4. Move TypeScript package discovery from `discover_ts_packages()` into `TypeScriptAnalyzer::discover()`.

### Deliverables

- [ ] Workspace-aware canonical path mapping
- [ ] Rust analyzer detects clap subcommands and emits command nodes
- [ ] `LanguageAnalyzer` trait extended with `can_analyze()` and `discover()`
- [ ] `analyze_project()` refactored to use `AnalyzerRegistry`
- [ ] 0 not-evaluable constraints in full conformance dog-food test
- [ ] All existing tests continue to pass

---

## Milestone 12: Additional Export Formats

**Priority:** Medium — extends the export pipeline through the existing `ExportRegistry`.

### DOT/Graphviz Exporter

Implement `ExportFormat` for DOT language output:
- Compound nodes as `subgraph cluster_*` blocks
- Edge styling by kind (depends = solid, contains = dashed, data_flow = dotted)
- Node labelling with name and kind
- Layout direction configurable (TB/LR)

**File:** `crates/core/src/export/dot.rs`

### SVG Exporter

Two options:
1. **External Graphviz:** Shell out to `dot -Tsvg` — requires Graphviz installed
2. **Embedded layout:** Use `layout-rs` crate for pure-Rust SVG generation

Recommended: Option 1 for now (simpler, higher quality output), with a fallback error if `dot` is not on PATH.

**File:** `crates/core/src/export/svg.rs` (wraps DOT exporter)

### Registration

Both formats registered in `ExportRegistry::with_defaults()`. CLI support is automatic through the registry-based dispatch already wired in M10.

### Deliverables

- [ ] `DotExporter` implementing `ExportFormat`
- [ ] `SvgExporter` implementing `ExportFormat` (via Graphviz)
- [ ] Snapshot tests for DOT output
- [ ] Registration in `ExportRegistry::with_defaults()`
- [ ] Dog-food: `svt export --format dot` produces valid DOT for this project

---

## Milestone 13: Snapshot Diffing + Git Integration

**Priority:** Medium — enables comparing architecture over time, a key value proposition.

### Core Diff Engine

New module `crates/core/src/diff.rs`:
- `diff_snapshots(store, v1, v2) -> SnapshotDiff`
- `SnapshotDiff` contains: `added_nodes`, `removed_nodes`, `changed_nodes`, `added_edges`, `removed_edges`
- Matching by canonical path (not by ID, since IDs are version-specific)
- "Changed" means same canonical path but different metadata, kind, or sub_kind

### CLI Command

`svt diff --from V1 --to V2` — prints a human-readable summary of changes. Optionally `--format json` for machine-readable output.

### Git Integration

Enhance `analyze_project()`:
- Auto-detect `HEAD` commit hash via `git rev-parse HEAD`
- Pass as `commit_ref` to `create_snapshot()`
- Store commit metadata (hash, author, date, message) in snapshot

### API + Web UI

- `GET /api/diff?from=V1&to=V2` returning `SnapshotDiff` as JSON
- Web UI: diff view highlighting added (green), removed (red), changed (amber) nodes on the graph

### Deliverables

- [ ] `diff_snapshots()` in core
- [ ] `svt diff` CLI command
- [ ] Git-aware snapshot creation
- [ ] `GET /api/diff` endpoint
- [ ] Web UI diff visualization
- [ ] Property tests for diff symmetry (diff(a,b) ≠ diff(b,a) but |added| and |removed| swap)

---

## Milestone 14: Web UI Polish

**Priority:** Medium — improves day-to-day usability.

### Dark Mode

- CSS custom properties for theme colours
- Toggle switch in header
- `prefers-color-scheme` media query for system default
- Cytoscape.js style sheet swap for dark theme

### URL Routing

- Hash-based routing: `#/node/{path}`, `#/view/{name}`, `#/layout/{mode}`
- Selected node reflected in URL — shareable links
- Back/forward navigation support

### Persistence

- `localStorage` for: layout preference, theme, filter state, last selected node
- Restore state on page load

### UX Improvements

- Loading spinner during initial data fetch
- Empty-state illustrations when no data loaded
- Error boundary with retry button
- Keyboard shortcuts: `Escape` to deselect, arrow keys for graph traversal, `/` to focus search

### Deliverables

- [ ] Dark mode with system preference detection
- [ ] Hash-based URL routing
- [ ] LocalStorage persistence
- [ ] Loading/empty/error states
- [ ] Keyboard shortcuts

---

## Milestone 15: Additional Language Analyzers

**Priority:** Low — extends reach but not needed for the core use case.

### Go Analyzer

- tree-sitter-go grammar
- Discovery: `go.mod` detection, package directory scanning
- Extraction: package → type → function → method hierarchy
- Canonical path: `module/package/type/function`

### Python Analyzer

- tree-sitter-python grammar
- Discovery: `pyproject.toml`, `setup.py`, or `__init__.py` detection
- Extraction: package → module → class → function hierarchy
- Canonical path: `package/module/class/function`

### Registration

Both registered in `AnalyzerRegistry::with_defaults()` (requires M11 trait extension first).

### Deliverables

- [ ] `GoAnalyzer` implementing extended `LanguageAnalyzer`
- [ ] `PythonAnalyzer` implementing extended `LanguageAnalyzer`
- [ ] Dog-food test on sample Go/Python projects
- [ ] Registration in default registry

---

## Milestone 16: Dynamic Plugin Loading

**Priority:** Low — the registry API from M10 supports programmatic extension; dynamic loading adds runtime extensibility but is a significant engineering effort.

### Plugin Manifest

```toml
# svt-plugin.toml
[plugin]
name = "svt-go-analyzer"
version = "0.1.0"
svt_api_version = "1"

[[provides]]
type = "language_analyzer"
language_id = "go"
```

### Discovery Conventions

- `~/.svt/plugins/*/svt-plugin.toml` — user-global plugins
- `.svt/plugins/*/svt-plugin.toml` — project-local plugins
- `SVT_PLUGIN_PATH` environment variable — additional search paths

### Loading

- Rust dylib plugins via `libloading`
- Each plugin exports a `register(registry: &mut PluginRegistrar)` function
- `PluginRegistrar` provides typed registration methods for each extension point

### CLI

- `svt plugin list` — show installed plugins
- `svt plugin install <path>` — copy plugin to user directory
- `svt plugin remove <name>` — remove plugin

### Security

- Dylib plugins run in-process — no sandboxing (same trust model as Rust libraries)
- API version check on load — reject incompatible plugins
- Checksum verification for installed plugins

### Deliverables

- [ ] Plugin manifest format
- [ ] Filesystem discovery
- [ ] `libloading`-based dynamic loading
- [ ] `PluginRegistrar` API
- [ ] `svt plugin` CLI subcommands
- [ ] Documentation for plugin authors

---

## Dependency Graph

```
M11 (Analyzer Wiring + Path Alignment)
 ├── M12 (Export Formats) — independent, can run in parallel
 ├── M13 (Diffing + Git) — independent, can run in parallel
 ├── M14 (Web UI Polish) — benefits from M13 diff API
 ├── M15 (Additional Analyzers) — depends on M11 trait extensions
 └── M16 (Dynamic Loading) — depends on M11 trait extensions, M15 validates the API
```

Recommended order: **M11 → (M12 ∥ M13) → M14 → M15 → M16**
