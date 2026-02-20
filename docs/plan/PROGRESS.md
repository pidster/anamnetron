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
| **13** | Snapshot Diffing + Git Integration | 2026-02-19 | 315 | Core diff engine (node/edge matching by canonical path), `svt diff --from V1 --to V2` (human + JSON output), `GET /api/diff?from=V1&to=V2` endpoint, git HEAD auto-detection in `svt analyze` |
| **14** | Web UI Polish | 2026-02-19 | 329 | Dark/light theme toggle, hash-based URL routing (`#v=1&node=id&layout=dagre`), localStorage persistence, keyboard navigation (Escape/f), loading spinner, empty state, `getDiff` API client + diff types |
| **15** | Additional Language Analyzers (Go + Python) | 2026-02-19 | 359 | tree-sitter-go/python analyzers, Go module + Python package discovery, `go.mod`/`pyproject.toml`/`setup.py` support, 6-phase analysis pipeline, 14 new Go/Python analyzer tests, 7 new discovery tests |
| **16** | Web UI Diff View + SVG/PNG Export | 2026-02-20 | 371 | Diff overlay on Cytoscape graph (added/removed/changed CSS classes), compare-to dropdown, diff summary banner, URL hash diff param; `SvgExporter`/`PngExporter` via Graphviz CLI piping, PNG binary handling in CLI |
| **17** | Dynamic Plugin Loading | 2026-02-20 | 388 | `SvtPlugin` trait + `declare_plugin!` macro in svt-core, `PluginLoader` with `libloading` in svt-cli, `--plugin` flag + `svt plugin list` command, plugin contributions wired into check/export, 3-tier discovery (CLI/project/user) |

**Current state:** 366 Rust tests + 22 vitest tests = 388 total. All passing. clippy/fmt/audit clean. CI pipeline operational.

## What's Working Now

```
svt import design/architecture.yaml     # Load a design model
svt check                                # Conformance check (design-only)
svt analyze .                            # Analyze Rust + TypeScript + Go + Python project
svt check --analysis                     # Compare design vs analysis
svt export --format mermaid              # Export as Mermaid flowchart
svt export --format json                 # Export as interchange JSON
svt export --format dot                  # Export as DOT (Graphviz)
svt export --format svg -o arch.svg      # Export as SVG (requires Graphviz)
svt export --format png -o arch.png      # Export as PNG (requires Graphviz)
svt export --format mermaid -o arch.mmd  # Export to file
svt diff --from 1 --to 2                 # Compare two snapshots (human output)
svt diff --from 1 --to 2 --format json   # Compare two snapshots (JSON output)
svt plugin list                          # List loaded plugins and their contributions
svt --plugin path/to/lib.dylib check     # Load a plugin and run conformance checks
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), conformance overlay, and diff view overlay for comparing snapshots. With WASM loaded, node detail lookups and search run entirely in the browser — zero API round-trips after initial snapshot load.

All 12 constraints in `design/architecture.yaml` are fully evaluated in both design-only and full conformance mode — zero `NotEvaluable`. Dog-food conformance: 12 passed, 0 failed, 0 warned, 0 not evaluable. There are 10 unimplemented design nodes (expected — some are future work like `/svt/web`) and ~518 undocumented analysis nodes (expected — analysis is much more granular than the design model).

## Known Gaps

### Analyzer Wiring — RESOLVED
- ~~`AnalyzerRegistry` exists (M10) but `analyze_project()` still hardcodes the Rust and TypeScript phases directly rather than dispatching through the registry.~~ Resolved by introducing `LanguageOrchestrator` trait with `OrchestratorRegistry`-based dispatch. `analyze_project()` now iterates over registered orchestrators with a uniform discover-analyse-postprocess pipeline. Four orchestrators implemented: Rust, TypeScript, Go, Python.

### Canonical Path Alignment — RESOLVED (M11)
- ~~4 dog-food constraints were "not evaluable" in conformance mode.~~ Fixed by workspace-aware canonical path mapping (`svt-core` → `svt::core` → `/svt/core`) and enum variant extraction. All 12 constraints now pass.

### Analysis Depth — PARTIALLY RESOLVED
- ~~The analyzer extracts crate/module/type/function structure but does not resolve cross-crate call graphs, method calls, or trait implementations.~~ `self.method()` calls inside `impl` blocks are now resolved by propagating the impl type through the tree-sitter walk (e.g., `self.baz()` inside `impl Foo` resolves to `Foo::baz`). Methods in impl blocks are parented under their type in the containment hierarchy. Non-self method calls (`x.foo()`) and cross-crate calls remain unresolved (~20 file-level warning summaries). This limits the accuracy of dependency-direction constraints for non-self calls.

### Export Formats — RESOLVED (M16)
- ~~Mermaid, JSON, and DOT are implemented. SVG/PNG rendering could be added via Graphviz CLI piping or embedded renderer.~~ SVG and PNG export added via Graphviz CLI piping (`SvgExporter`, `PngExporter`). All five formats (Mermaid, JSON, DOT, SVG, PNG) available.

### Web UI — RESOLVED (M16)
- ~~No dark mode, no persistence of layout/filter state, no URL routing/permalinks.~~ Dark/light theme toggle, hash-based URL routing, localStorage persistence, keyboard shortcuts, and diff view overlay all implemented.

### Additional Languages — PARTIALLY RESOLVED (M15)
- ~~Only Rust and TypeScript analyzers exist.~~ Go and Python analyzers added in M15 with tree-sitter grammars. Java and other languages remain as future goals (PRINCIPLES.md: Extensibility).

### Git Integration — RESOLVED (M13 + M16)
- ~~`analyze_project()` accepts an optional `commit_ref` but there is no automatic git-aware snapshot creation or change detection.~~ `svt analyze` now auto-detects git HEAD when `--commit-ref` is not provided. Change detection between snapshots is available via `svt diff`. Web UI diff view overlay added in M16.

### Dynamic Plugin Loading — RESOLVED (M17)
- ~~Plugin registries exist with `.register()` API but all plugins are compiled in. No external plugin discovery, no dynamic loading, no plugin manifest format.~~ Resolved: `SvtPlugin` trait + `declare_plugin!` macro, `PluginLoader` with `libloading`, `--plugin` flag, `svt plugin list`, 3-tier discovery (CLI/project-local/user-global). Plugin manifest format (`svt-plugin.toml`) and install/remove commands remain as future work.

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

**Not yet done (deferred):** ~~`AnalyzerRegistry`-based dispatch~~ — Resolved in post-M15 gap cleanup via `LanguageOrchestrator` trait and `OrchestratorRegistry`.

### Milestone 12: DOT Export — COMPLETE

**Goal:** Add DOT (Graphviz) export format through the `ExportRegistry`.

**Delivered:**
- `DotExporter` implementing `ExportFormat` trait with `subgraph cluster_*` containment
- Labelled directed edges for non-containment relationships
- Registered in `ExportRegistry::with_defaults()` — CLI picks it up automatically
- 3 unit tests + 1 snapshot test + 1 CLI integration test
- `svt export --format dot` works out of the box

**Not yet done (deferred):** ~~SVG rendering could be added via Graphviz CLI piping or an embedded renderer.~~ Resolved in M16 with `SvgExporter` and `PngExporter`.

### Milestone 13: Snapshot Diffing + Git Integration — COMPLETE

**Goal:** Enable comparing two analysis snapshots and integrate with git for automatic version tracking.

**Delivered:**
- Core diff engine: nodes matched by canonical path, edges by (source, target, kind) tuple
- `SnapshotDiff` with `NodeChange`, `EdgeChange`, `DiffSummary` types (Serialize + Deserialize)
- `svt diff --from V1 --to V2` with human-readable and JSON output
- `GET /api/diff?from=V1&to=V2` server endpoint
- Git HEAD auto-detection in `svt analyze` (shells out to `git rev-parse HEAD`)
- 9 core diff tests + 2 CLI integration tests + 2 server tests

**Not yet done (deferred):** ~~Web UI diff view — highlight added/removed/changed nodes in graph overlay.~~ Resolved in M16.

### Milestone 14: Web UI Polish — COMPLETE

**Goal:** Improve the web frontend with dark mode, persistence, URL routing, and better UX.

**Delivered:**
- Dark/light theme toggle with CSS custom properties and localStorage persistence
- Theme-aware Cytoscape graph (colors adapt to light/dark mode)
- Hash-based URL routing (`#v=1&node=id&layout=dagre`) with back/forward support
- localStorage persistence for layout preference and theme
- Keyboard navigation: Escape to close panels, `f` to fit-all
- Loading spinner animation and improved empty state messaging
- `getDiff` API client function and `SnapshotDiff` TypeScript types
- 13 new router tests + 1 new API test = 19 total vitest tests

**Not yet done (deferred):** Error boundary components with retry, ~~diff view overlay in graph~~ (resolved in M16), arrow-key graph traversal.

### Milestone 15: Additional Language Analyzers — COMPLETE

**Goal:** Add Go and Python analyzers through the `AnalyzerRegistry`.

**Delivered:**
- `GoAnalyzer` with tree-sitter-go: function, method (with receiver type), struct, interface, type alias, import extraction
- `PythonAnalyzer` with tree-sitter-python: class, function, method, decorated definition, import/import-from extraction
- Go module discovery via `go.mod` with package directory walking (excludes `vendor/`, `_test.go`)
- Python package discovery via `pyproject.toml` [project] name and `setup.py` name= fallback (excludes venv, .venv, __pycache__)
- Both registered in `AnalyzerRegistry::with_defaults()`
- 6-phase analysis pipeline: Rust → TypeScript → Go → Python → Mapping → Insertion
- 14 new analyzer unit tests (7 Go + 7 Python), 7 new discovery tests
- CLI output updated to show Go module and Python package counts

**Not yet done (deferred):** Java and other languages not yet supported.

### Known Gaps Cleanup (Post-M15)

**Goal:** Clean up three known gaps before proceeding to the next milestone.

**Delivered:**
- `LanguageOrchestrator` trait with `OrchestratorRegistry` — uniform discover-analyse-postprocess pipeline replacing hardcoded per-language phases in `analyze_project()`
- Four orchestrators: `RustOrchestrator`, `TypeScriptOrchestrator`, `GoOrchestrator`, `PythonOrchestrator`
- TypeScript orchestrator handles complex post-processing (item reparenting, import resolution) via `emit_structural_items()` and `post_process()` overrides
- Rust orchestrator handles workspace root emission via `extra_items()` override
- Method-call warning aggregation: one summary per file instead of ~3,500 individual warnings
- Project root validation in `analyze_project()` for better error reporting

### Milestone 16: Web UI Diff View + SVG/PNG Export — COMPLETE

**Goal:** Add diff overlay visualization to the web UI and SVG/PNG export via Graphviz CLI piping.

**Delivered:**
- `SvgExporter` and `PngExporter` implementing `ExportFormat` trait, piping DOT through `dot -Tsvg`/`dot -Tpng`
- PNG binary handling in CLI (`--output` required, writes raw bytes)
- Graceful error when Graphviz `dot` is not installed
- Diff overlay on Cytoscape graph: `.diff-added` (green dashed), `.diff-removed` (red dashed, faded), `.diff-changed` (amber dashed)
- "Compare to..." dropdown in toolbar for selecting comparison snapshot
- Diff summary banner showing added/removed/changed node counts
- `diff` parameter in URL hash routing for permalinks (`#v=2&diff=1`)
- Diff state in graph store (`diffReport`, `diffVersion`, `clearDiff()`)
- 2 new Rust export tests + 3 new vitest router tests
- **Result: 349 Rust tests + 22 vitest tests = 371 total**

### Milestone 17: Dynamic Plugin Loading — COMPLETE

**Goal:** Support external plugins loaded at runtime from the filesystem.

**Delivered:**
- `SvtPlugin` trait in svt-core: `name()`, `version()`, `api_version()`, `constraint_evaluators()`, `export_formats()`
- `SVT_PLUGIN_API_VERSION` constant (v1) for host/plugin compatibility checking
- `declare_plugin!` macro generating `extern "C" fn svt_plugin_create()` entry point
- `PluginError` enum (`LoadFailed`, `SymbolNotFound`, `ApiVersionMismatch`) with `thiserror`
- `PluginLoader` in svt-cli using `libloading` for dynamic loading (WASM-safe: `libloading` not in svt-core)
- Null-pointer defensive check before `Box::from_raw` in plugin entry point
- 3-tier plugin discovery: `--plugin` CLI flag, `.svt/plugins/`, `~/.svt/plugins/`
- `svt plugin list` subcommand showing loaded plugins and their contributions
- Plugin contributions wired into `svt check` and `svt export` via `register_all()`
- Load failures are non-fatal warnings (stderr), never abort execution
- 15 plugin unit tests (7 in svt-core, 8 in svt-cli) + 2 CLI integration tests
- **Result: 366 Rust tests + 22 vitest tests = 388 total**

**Not yet done (deferred):** Plugin manifest format (`svt-plugin.toml`), `svt plugin install|remove` commands, plugin sandboxing, `LanguageOrchestrator` support in plugin API (blocked by inward dependency rule — orchestrator lives in svt-analyzer).

## Roadmap (Post-M17)

Priority-ordered next milestones:

| # | Milestone | Description | Key Challenge |
|---|-----------|-------------|---------------|
| **M18** | Plugin Analyzer Support | Expose `LanguageOrchestrator` in the plugin API so external plugins can contribute language analyzers | Inward dependency rule: `LanguageOrchestrator` lives in svt-analyzer, plugins depend on svt-core. Need to either extract an analyzer trait to svt-core or introduce a thin interface crate. |
| **M19** | Store Persistence | On-disk CozoDB backend so analysis results survive across CLI sessions | CozoDB supports RocksDB-backed storage; need to add store path configuration, migration, and open/create lifecycle to `GraphStore` |
| **M20** | Incremental Analysis | Diff changed files and update only affected subgraphs instead of full re-analysis | Requires file-level change detection (git diff or mtime), dependency graph for invalidation, partial store updates |
| **M21** | Analysis Depth | Resolve non-self method calls (`x.foo()`), cross-crate dependency edges | Needs type inference or heuristic resolution; significantly harder than self-method resolution |
| **M22** | Plugin Ecosystem | Plugin manifest (`svt-plugin.toml`), `svt plugin install\|remove`, plugin author documentation | Discovery conventions, version compatibility, documentation site |
| **M23** | Web UI Enhancements | Error boundaries with retry, arrow-key graph traversal, filtering sidebar (by kind/metadata) | UX design, Cytoscape keyboard integration |
| **M24** | Additional Languages | Java analyzer (tree-sitter-java), others as community demand dictates | tree-sitter-java grammar, Maven/Gradle project discovery |

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
| `2026-02-19-milestone-13-implementation.md` | M13 implementation plan (COMPLETE) |
| `2026-02-19-milestone-14-implementation.md` | M14 implementation plan (COMPLETE) |
| `2026-02-20-milestone-15-implementation.md` | M15 implementation plan (COMPLETE) |
| `2026-02-20-diff-view-svg-export-design.md` | M16 design (diff view + SVG/PNG export) |
| `2026-02-20-diff-view-svg-export-implementation.md` | M16 implementation plan (COMPLETE) |
| `2026-02-19-milestones-11-16-design.md` | M11–M16 design (roadmap for remaining work) |
| `2026-02-20-analysis-depth-design.md` | Analysis depth: Rust self.method() resolution design |
| `2026-02-20-analysis-depth-implementation.md` | Analysis depth: Rust self.method() resolution implementation plan |
| `2026-02-20-dynamic-plugin-loading-design.md` | M17 design (dynamic plugin loading) |
| `2026-02-20-dynamic-plugin-loading-implementation.md` | M17 implementation plan (COMPLETE) |
