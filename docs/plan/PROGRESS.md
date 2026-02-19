# Software Visualizer Tool — Progress & Roadmap

## Completed Milestones

| Milestone | Description | Date | Tests | Key Commits |
|-----------|-------------|------|-------|-------------|
| **1** | Core Data Model + CozoDB Store | 2026-02-15 | 84 | Node/Edge/Snapshot types, GraphStore trait, CozoDB backend, containment/dependency queries, proptest |
| **2** | Interchange Format, Conformance, CLI | 2026-02-15 | 143 | Canonical paths, YAML/JSON import/export, `must_not_depend` constraint, `svt import`, `svt check`, dog-food |
| **3** | Rust Analyzer + Discovery Mode | 2026-02-17 | 201 | tree-sitter-rust analysis, crate/module/type/function extraction, `svt analyze`, conformance comparison |
| **4** | Server API (Axum) | 2026-02-17 | 218 | 13 REST endpoints, Cytoscape.js graph format, conformance endpoints, search, integration tests |
| **5** | Svelte Web Frontend | 2026-02-17 | 224 | Svelte 5 + Cytoscape.js, compound nodes, conformance overlay, node detail panel, static serving |
| **6** | CLI Export + Additional Constraints | 2026-02-19 | 234 | `boundary`, `must_contain`, `max_fan_in` constraints; `svt export --format mermaid\|json`; Mermaid flowchart generation; all dog-food constraints evaluable |
| **7** | TypeScript Analyzer | 2026-02-19 | 259 | tree-sitter-typescript analysis, Svelte script block extraction, TS package discovery, multi-language orchestrator integration, dog-food on `web/` |
| **8** | WASM Bridge | 2026-02-19 | 282 | `svt-wasm` crate with wasm-bindgen, CozoDB in-memory for browser, 12 read-only query methods, TypeScript wrapper, web integration for zero-roundtrip detail lookups |
| **9** | CI Pipeline | 2026-02-19 | 282 | GitHub Actions CI: Rust fmt/clippy/test/audit, WASM build, web tests, conformance gate with step summary |
| **10** | Plugin Foundations | 2026-02-19 | 282 | `ConstraintEvaluator`, `ExportFormat`, `LanguageAnalyzer` traits; `ConstraintRegistry`, `ExportRegistry`, `AnalyzerRegistry` with `with_defaults()` + `.register()`; registry-based dispatch in CLI, server, conformance engine; `&dyn GraphStore` migration |

**Current state:** 277 Rust tests + 5 vitest tests = 282 total. All passing. clippy/fmt/audit clean. CI pipeline operational. Plugin registries wired end-to-end.

## What's Working Now

```
svt import design/architecture.yaml     # Load a design model
svt check                                # Conformance check (design-only)
svt analyze .                            # Analyze Rust + TypeScript project with tree-sitter
svt check --analysis                     # Compare design vs analysis
svt export --format mermaid              # Export as Mermaid flowchart
svt export --format json                 # Export as interchange JSON
svt export --format mermaid -o arch.mmd  # Export to file
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), and conformance overlay. With WASM loaded, node detail lookups and search run entirely in the browser — zero API round-trips after initial snapshot load.

All 10 constraints in `design/architecture.yaml` are now fully evaluated — zero `NotEvaluable` (design-only mode). In full conformance mode (design vs analysis), 4 constraints are not evaluable because the analyzer does not yet produce nodes at the exact canonical paths expected by the design model (e.g., `/svt/cli/commands` for `must_contain` and `/svt/core/model` for `max_fan_in`).

## Known Gaps

### Analyzer Wiring
- `AnalyzerRegistry` exists (M10) but `analyze_project()` still hardcodes the Rust and TypeScript phases directly rather than dispatching through the registry. The `LanguageAnalyzer` trait needs `can_analyze()` and `discover()` methods before registry-based dispatch is viable.

### Canonical Path Alignment
- 4 dog-food constraints are "not evaluable" in conformance mode because analysis nodes don't align with design model paths. The Rust analyzer produces crate-level nodes (e.g., `/svt/svt-cli`) but the design model expects logical paths (e.g., `/svt/cli/commands/check`). A mapping layer is needed to bridge the gap.

### Analysis Depth
- The analyzer extracts crate/module/type/function structure but does not resolve cross-crate call graphs, method calls, or trait implementations. ~3,500 warnings are generated during dog-food analysis (mostly "method call resolution not yet supported"). This limits the accuracy of dependency-direction constraints.

### Export Formats
- Only Mermaid and JSON are implemented. The design mentions SVG/PNG and DOT/Graphviz as goals (PRINCIPLES.md: Interoperability).

### Web UI
- No dark mode, no persistence of layout/filter state, no diff view for comparing snapshots, no URL routing/permalinks.

### Additional Languages
- Only Rust and TypeScript analyzers exist. Go, Python, and Java are mentioned as future goals (PRINCIPLES.md: Extensibility).

### Git Integration
- `analyze_project()` accepts an optional `commit_ref` but there is no automatic git-aware snapshot creation or change detection.

### Dynamic Plugin Loading
- Plugin registries exist with `.register()` API but all plugins are compiled in. No external plugin discovery, no dynamic loading, no plugin manifest format.

## Suggested Next Milestones

### Milestone 11: Analyzer Registry Wiring + Canonical Path Alignment

**Goal:** Wire `AnalyzerRegistry` into the analysis pipeline and fix the 4 not-evaluable dog-food constraints by aligning analysis canonical paths with the design model.

**Scope:**
- Extend `LanguageAnalyzer` trait with `can_analyze(&self, file: &Path) -> bool` and `discover(&self, root: &Path) -> Vec<ProjectUnit>` methods
- Refactor `analyze_project()` to iterate registered analyzers instead of hardcoding Rust/TS phases
- Add a configurable canonical path mapping layer between design paths and analysis paths
- Fix `must_contain` constraints for `/svt/cli/commands` by emitting function-level nodes for CLI commands
- Fix `max_fan_in` constraint for `/svt/core/model` by aligning module paths
- Target: 0 not-evaluable constraints in full conformance mode

### Milestone 12: Additional Export Formats

**Goal:** Add DOT/Graphviz and SVG export formats through the `ExportRegistry`.

**Scope:**
- DOT exporter implementing the `ExportFormat` trait (subgraph nesting for compound nodes, edge styling)
- SVG exporter via DOT-to-SVG pipeline (using Graphviz CLI or embedded renderer)
- Register in `ExportRegistry::with_defaults()`
- CLI `svt export --format dot|svg` support (automatic through registry)

### Milestone 13: Snapshot Diffing + Git Integration

**Goal:** Enable comparing two analysis snapshots and integrate with git for automatic version tracking.

**Scope:**
- Core diff engine: compute added/removed/changed nodes and edges between two versions
- `svt diff` CLI command comparing two snapshots
- Git-aware analysis: auto-detect HEAD commit, store as snapshot metadata
- API endpoint: `GET /api/diff?from=V1&to=V2`
- Web UI diff view: highlight added/removed/changed nodes in graph

### Milestone 14: Web UI Polish

**Goal:** Improve the web frontend with dark mode, persistence, URL routing, and better UX.

**Scope:**
- Dark mode toggle with system-preference detection
- URL routing (hash-based): selected node, active view, layout mode
- LocalStorage persistence for layout preferences and filter state
- Error boundary components with retry
- Loading states and empty-state UI
- Keyboard navigation (arrow keys to traverse graph, Escape to deselect)

### Milestone 15: Additional Language Analyzers

**Goal:** Add Go and Python analyzers through the `AnalyzerRegistry`.

**Scope:**
- Go analyzer: tree-sitter-go, package/type/function extraction, `go.mod` discovery
- Python analyzer: tree-sitter-python, package/module/class/function extraction, `pyproject.toml`/`setup.py` discovery
- Register in `AnalyzerRegistry::with_defaults()`
- Dog-food tests for each new analyzer on sample projects

### Milestone 16: Dynamic Plugin Loading

**Goal:** Support external plugins loaded at runtime from the filesystem.

**Scope:**
- Plugin manifest format (`svt-plugin.toml` or similar)
- Filesystem discovery conventions (`~/.svt/plugins/`, project-local `.svt/plugins/`)
- Dynamic loading via `libloading` for Rust dylib plugins
- CLI `svt plugin list|install|remove` commands
- Security considerations: plugin sandboxing, version compatibility

## Plan Documents

| Document | Content |
|----------|---------|
| `2026-02-15-milestone-1-core-data-model-design.md` | M1 design |
| `2026-02-15-milestone-1-core-implementation.md` | M1 implementation plan |
| `2026-02-15-milestone-2-design.md` | M2 design |
| `2026-02-15-milestone-2-implementation.md` | M2 implementation plan |
| `2026-02-17-milestone-3-analyzer-design.md` | M3 design |
| `2026-02-17-milestone-3-implementation.md` | M3 implementation plan |
| `2026-02-17-milestones-4-5-design.md` | M4+M5 design |
| `2026-02-17-milestone-4-implementation.md` | M4 implementation plan (COMPLETE) |
| `2026-02-17-milestone-5-implementation.md` | M5 implementation plan (COMPLETE) |
| `2026-02-19-milestone-6-design.md` | M6 design (COMPLETE) |
| `2026-02-19-milestone-6-implementation.md` | M6 implementation plan (COMPLETE) |
| `2026-02-19-milestone-7-design.md` | M7 design (COMPLETE) |
| `2026-02-19-milestone-7-implementation.md` | M7 implementation plan (COMPLETE) |
| `2026-02-19-milestone-8-design.md` | M8 design (COMPLETE) |
| `2026-02-19-milestone-8-implementation.md` | M8 implementation plan (COMPLETE) |
| `2026-02-19-milestone-9-design.md` | M9 design (COMPLETE) |
| `2026-02-19-milestone-9-implementation.md` | M9 implementation plan (COMPLETE) |
| `2026-02-19-milestone-10-design.md` | M10 design (COMPLETE) |
| `2026-02-19-milestone-10-implementation.md` | M10 implementation plan (COMPLETE) |
| `2026-02-19-milestones-11-16-design.md` | M11–M16 design (roadmap for remaining work) |
