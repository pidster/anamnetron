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
| **11** | Canonical Path Alignment | 2026-02-19 | 293 | Workspace-aware canonical paths (`svt-core` → `/svt/core`), enum variant extraction, workspace root node, 0 not-evaluable constraints in full conformance mode |
| **12** | DOT Export | 2026-02-19 | 302 | `DotExporter` implementing `ExportFormat` trait, `subgraph cluster_*` containment, labelled edges, registered in `ExportRegistry`, `svt export --format dot`, snapshot test |

**Current state:** 297 Rust tests + 5 vitest tests = 302 total. All passing. clippy/fmt/audit clean. CI pipeline operational.

## What's Working Now

```
svt import design/architecture.yaml     # Load a design model
svt check                                # Conformance check (design-only)
svt analyze .                            # Analyze Rust + TypeScript project with tree-sitter
svt check --analysis                     # Compare design vs analysis
svt export --format mermaid              # Export as Mermaid flowchart
svt export --format json                 # Export as interchange JSON
svt export --format dot                  # Export as DOT (Graphviz)
svt export --format mermaid -o arch.mmd  # Export to file
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), and conformance overlay. With WASM loaded, node detail lookups and search run entirely in the browser — zero API round-trips after initial snapshot load.

All 12 constraints in `design/architecture.yaml` are fully evaluated in both design-only and full conformance mode — zero `NotEvaluable`. Dog-food conformance: 12 passed, 0 failed, 0 warned, 0 not evaluable. There are 10 unimplemented design nodes (expected — some are future work like `/svt/web`) and ~518 undocumented analysis nodes (expected — analysis is much more granular than the design model).

## Known Gaps

### Analyzer Wiring
- `AnalyzerRegistry` exists (M10) but `analyze_project()` still hardcodes the Rust and TypeScript phases directly rather than dispatching through the registry. The `LanguageAnalyzer` trait needs `can_analyze()` and `discover()` methods before registry-based dispatch is viable.

### Canonical Path Alignment — RESOLVED (M11)
- ~~4 dog-food constraints were "not evaluable" in conformance mode.~~ Fixed by workspace-aware canonical path mapping (`svt-core` → `svt::core` → `/svt/core`) and enum variant extraction. All 12 constraints now pass.

### Analysis Depth
- The analyzer extracts crate/module/type/function structure but does not resolve cross-crate call graphs, method calls, or trait implementations. ~3,500 warnings are generated during dog-food analysis (mostly "method call resolution not yet supported"). This limits the accuracy of dependency-direction constraints.

### Export Formats
- Mermaid, JSON, and DOT are implemented. SVG/PNG rendering could be added via Graphviz CLI piping or embedded renderer (PRINCIPLES.md: Interoperability).

### Web UI
- No dark mode, no persistence of layout/filter state, no diff view for comparing snapshots, no URL routing/permalinks.

### Additional Languages
- Only Rust and TypeScript analyzers exist. Go, Python, and Java are mentioned as future goals (PRINCIPLES.md: Extensibility).

### Git Integration
- `analyze_project()` accepts an optional `commit_ref` but there is no automatic git-aware snapshot creation or change detection.

### Dynamic Plugin Loading
- Plugin registries exist with `.register()` API but all plugins are compiled in. No external plugin discovery, no dynamic loading, no plugin manifest format.

## Suggested Next Milestones

### Milestone 11: Canonical Path Alignment — COMPLETE

**Goal:** Fix the 4 not-evaluable dog-food constraints by aligning analysis canonical paths with the design model.

**Delivered:**
- Workspace name detection from common crate name prefix (`svt-core`, `svt-cli` → workspace `svt`)
- Workspace-aware qualified name mapping (`svt-core` → `svt::core` → `/svt/core`)
- Workspace root node emission (`/svt` as `System`/`workspace`)
- Enum variant extraction via tree-sitter (enabling `must_contain` constraints)
- Binary target naming fix (always use package name, not target name)
- **Result: 12 passed, 0 failed, 0 warned, 0 not evaluable**

**Not yet done (deferred):** `AnalyzerRegistry`-based dispatch — `analyze_project()` still hardcodes Rust/TS phases. Registry wiring deferred until additional language analyzers (M15) make it necessary.

### Milestone 12: DOT Export — COMPLETE

**Goal:** Add DOT (Graphviz) export format through the `ExportRegistry`.

**Delivered:**
- `DotExporter` implementing `ExportFormat` trait with `subgraph cluster_*` containment
- Labelled directed edges for non-containment relationships
- Registered in `ExportRegistry::with_defaults()` — CLI picks it up automatically
- 3 unit tests + 1 snapshot test + 1 CLI integration test
- `svt export --format dot` works out of the box

**Not yet done (deferred):** SVG rendering could be added via Graphviz CLI piping or an embedded renderer.

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
