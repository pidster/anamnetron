# Anamnetron — Progress & Roadmap

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
| **18** | Plugin Analyzer Support | 2026-02-20 | 404 | `LanguageDescriptor` + `LanguageParser` trait in svt-core, `DescriptorOrchestrator` in svt-analyzer, Go/Python/TypeScript refactored to descriptor+parser pattern, `SvtPlugin::language_parsers()` method, plugin parsers wired into CLI analysis pipeline |
| **19** | Store Persistence & Management | 2026-02-20 | 420 | Schema version + migration framework, `store_info()` with per-snapshot counts, `svt store info\|compact\|reset` CLI commands, `--store` flag for server persistent storage, `GET /api/store/info` endpoint |
| **20** | Incremental Analysis | 2026-02-20 | 453 | BLAKE3 file hashing, `file_manifest` relation, `copy_nodes`/`copy_edges` store methods, unit-level skip with copy-then-upsert, `svt analyze --incremental`, proptest for manifest diffing |
| **21** | Analysis Depth | 2026-02-21 | 470 | Crate-level `Depends` edges from Cargo metadata, `Self::method()` and `Type::method()` resolution, heuristic local variable type inference (`let x: Foo`, `Foo::new()`, struct expressions, function params), method call resolution statistics |
| **22** | Plugin Ecosystem | 2026-02-21 | 506 | Plugin manifest format (`svt-plugin.toml`), `svt plugin install\|remove\|info` commands, manifest-aware plugin loading with source tracking, sidecar manifest discovery, plugin authoring documentation |
| **23** | Web UI Enhancements | 2026-02-21 | 533 | Error boundaries with retry, arrow-key graph traversal (Up/Down/Left/Right for containment hierarchy), filtering sidebar (node kind, edge kind, sub-kind, language) |
| **24** | Multi-Tenancy (Project-Scoped Store) | 2026-02-27 | 763 | CozoDB schema v2 migration, `projects`+`snapshot_projects` relations, project CRUD API, `svt push` CLI command, web UI project selector, per-project versioning |
| **25** | Test Detection (All Languages) | 2026-03-04 | — | Tag test code in TS (`*.test.ts`/`*.spec.ts`/`__tests__`), Go (`_test.go`, `Test*/Bench*/Example*/Fuzz*`), Python (`test_*.py`/`*_test.py`/`conftest.py`, `test_*` funcs, `Test*` classes) |
| **26** | Module Hierarchy & Post-Processing (Go + Python) | 2026-03-04 | — | Go directory-level module nodes, Python file+directory modules with `__init__.py` → package QN, relative import resolution via `post_process()` |
| **27** | TypeScript Structural Depth | 2026-03-04 | — | Class members (methods/properties/constructors), interface members, enum members/variants, `Extends`/`Implements` heritage edges, non-exported item extraction |
| **28** | Call Graph Analysis (TS + Go + Python) | 2026-03-04 | — | `visit_ts_call_expressions`, `visit_go_call_expressions` (import alias resolution), `visit_py_call_expressions` (self.method resolution) |
| **29** | Cross-Package Dependency Extraction | 2026-03-04 | 837 | `workspace_dependencies` on `LanguageUnit`, manifest parsing (package.json/go.mod/pyproject.toml), workspace-internal filtering, `Depends` edge emission |

**Current state:** 643 Rust tests + 194 vitest tests = 837 total. All passing. clippy/fmt/audit clean. CI pipeline operational.

## What's Working Now

```
svt import design/architecture.yaml     # Load a design model
svt check                                # Conformance check (design-only)
svt analyze .                            # Analyze Rust + TypeScript + Go + Python project
svt analyze . --incremental              # Incremental analysis (skip unchanged units)
svt check --analysis                     # Compare design vs analysis
svt export --format mermaid              # Export as Mermaid flowchart
svt export --format json                 # Export as interchange JSON
svt export --format dot                  # Export as DOT (Graphviz)
svt export --format svg -o arch.svg      # Export as SVG (requires Graphviz)
svt export --format png -o arch.png      # Export as PNG (requires Graphviz)
svt export --format mermaid -o arch.mmd  # Export to file
svt diff --from 1 --to 2                 # Compare two snapshots (human output)
svt diff --from 1 --to 2 --format json   # Compare two snapshots (JSON output)
svt store info                           # Show store schema version, snapshots, node/edge counts
svt store compact                        # Remove old versions (keep latest design + analysis)
svt store compact --keep 1 --keep 3      # Keep specific versions
svt store reset --force                  # Delete and recreate the store
svt plugin list                          # List loaded plugins and their contributions
svt plugin install /path/to/plugin       # Install plugin from directory with svt-plugin.toml
svt plugin install /path/to/plugin --global  # Install to user-global ~/.svt/plugins/
svt plugin remove svt-plugin-foo         # Remove installed plugin
svt plugin info /path/to/plugin          # Show plugin manifest metadata
svt --plugin path/to/lib.dylib check     # Load a plugin and run conformance checks
svt push --server http://host:3000       # Push analysis to remote server
svt push --server http://host:3000 --project foo  # Push to specific project
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
svt-server --store .svt/store            # Serve with persistent storage (data survives restart)
svt-server --store .svt/store --design design/architecture.yaml
                                         # Persistent store + fresh design import at startup
svt-server --project myapp              # Scope to a named project (default: "default")
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), conformance overlay, diff view overlay, error boundaries with retry, arrow-key graph traversal, a filtering sidebar for node/edge/sub-kind/language filtering, and a project selector (visible when multiple projects exist). With WASM loaded, node detail lookups and search run entirely in the browser — zero API round-trips after initial snapshot load.

All 12 constraints in `design/architecture.yaml` are fully evaluated in both design-only and full conformance mode — zero `NotEvaluable`. Dog-food conformance: 12 passed, 0 failed, 0 warned, 0 not evaluable. There are 10 unimplemented design nodes (expected — some are future work like `/svt/web`) and ~518 undocumented analysis nodes (expected — analysis is much more granular than the design model).

## Known Gaps

### Analyzer Wiring — RESOLVED
- ~~`AnalyzerRegistry` exists (M10) but `analyze_project()` still hardcodes the Rust and TypeScript phases directly rather than dispatching through the registry.~~ Resolved by introducing `LanguageOrchestrator` trait with `OrchestratorRegistry`-based dispatch. `analyze_project()` now iterates over registered orchestrators with a uniform discover-analyse-postprocess pipeline. Four orchestrators implemented: Rust, TypeScript, Go, Python.

### Canonical Path Alignment — RESOLVED (M11)
- ~~4 dog-food constraints were "not evaluable" in conformance mode.~~ Fixed by workspace-aware canonical path mapping (`svt-core` → `svt::core` → `/svt/core`) and enum variant extraction. All 12 constraints now pass.

### Analysis Depth — PARTIALLY RESOLVED (M21)
- ~~The analyzer extracts crate/module/type/function structure but does not resolve cross-crate call graphs, method calls, or trait implementations.~~ `self.method()` calls inside `impl` blocks are resolved by propagating the impl type through the tree-sitter walk. `Self::method()` and local `Type::method()` associated function calls are resolved via scope-aware rewriting. Heuristic local variable type inference resolves method calls on variables with known types (explicit annotations, constructor patterns like `Foo::new()`, struct expressions, function parameters including `&`/`&mut` stripping). Crate-level `Depends` edges are extracted from Cargo metadata workspace dependencies. Dog-food: 468 of 3997 method calls resolved (11.7%). Chained calls (`x.foo().bar()`), trait objects, generics, closures, and cross-function type flow remain unresolved.

### Export Formats — RESOLVED (M16)
- ~~Mermaid, JSON, and DOT are implemented. SVG/PNG rendering could be added via Graphviz CLI piping or embedded renderer.~~ SVG and PNG export added via Graphviz CLI piping (`SvgExporter`, `PngExporter`). All five formats (Mermaid, JSON, DOT, SVG, PNG) available.

### Web UI — RESOLVED (M16 + M23)
- ~~No dark mode, no persistence of layout/filter state, no URL routing/permalinks.~~ Dark/light theme toggle, hash-based URL routing, localStorage persistence, keyboard shortcuts, diff view overlay, error boundaries with retry, arrow-key graph traversal, and filtering sidebar all implemented.

### Additional Languages — PARTIALLY RESOLVED (M15)
- ~~Only Rust and TypeScript analyzers exist.~~ Go and Python analyzers added in M15 with tree-sitter grammars. Java and other languages remain as future goals (PRINCIPLES.md: Extensibility).

### Analyzer Feature Parity — RESOLVED (M25–M29)
- ~~The Rust analyzer is significantly more capable than the TypeScript, Go, and Python analyzers.~~ Resolved: test detection added for all languages (M25), module hierarchy for Go and Python (M26), TypeScript structural depth including class/enum members and heritage edges (M27), call graph analysis for all non-Rust languages (M28), cross-package dependency extraction from build tool manifests (M29). See updated parity matrix below.

### Git Integration — RESOLVED (M13 + M16)
- ~~`analyze_project()` accepts an optional `commit_ref` but there is no automatic git-aware snapshot creation or change detection.~~ `svt analyze` now auto-detects git HEAD when `--commit-ref` is not provided. Change detection between snapshots is available via `svt diff`. Web UI diff view overlay added in M16.

### Dynamic Plugin Loading — RESOLVED (M17 + M22)
- ~~Plugin registries exist with `.register()` API but all plugins are compiled in. No external plugin discovery, no dynamic loading, no plugin manifest format.~~ Resolved: `SvtPlugin` trait + `declare_plugin!` macro, `PluginLoader` with `libloading`, `--plugin` flag, `svt plugin list`, 3-tier discovery (CLI/project-local/user-global). ~~Plugin manifest format (`svt-plugin.toml`) and install/remove commands remain as future work.~~ Resolved in M22: `svt-plugin.toml` manifest format, `svt plugin install|remove|info` commands, manifest-aware loading with source tracking, plugin authoring documentation.

### Plugin Analyzer Support — RESOLVED (M18)
- ~~`LanguageOrchestrator` lives in svt-analyzer; plugins depend on svt-core only. External plugins cannot contribute language analyzers.~~ Resolved: `LanguageDescriptor` struct + `LanguageParser` trait in svt-core (WASM-compatible), `DescriptorOrchestrator` adapter in svt-analyzer, `SvtPlugin::language_parsers()` method. Go, Python, TypeScript refactored to descriptor+parser pattern. Plugin language contributions wired into CLI analysis pipeline via `analyze_project_with_registry()`.

### Store Persistence — RESOLVED (M19)
- ~~The server always uses `CozoStore::new_in_memory()`, losing all data on restart. No schema migration system, no store management CLI commands, no way to inspect or compact the store.~~ Resolved: Schema version + migration framework (`CURRENT_SCHEMA_VERSION`, `schema_version()`, `migrate()`), `store_info()` with per-snapshot node/edge counts, `svt store info|compact|reset` CLI commands, `--store` flag for server persistent CozoDB storage, `GET /api/store/info` endpoint.

### Multi-Tenancy — RESOLVED (M24)
- ~~The server supports only a single project per store instance. No way to host multiple projects or push analysis results from a remote CLI.~~ Resolved: project-scoped store with CozoDB schema v2 migration, project CRUD API, per-project versioning, CLI `svt push` command, web UI project selector. Legacy routes default to `"default"` project for backwards compatibility.

### Incremental Analysis — RESOLVED (M20)
- ~~Each `svt analyze` run re-parses every source file. For large codebases this is wasteful when only a few files changed.~~ Resolved: BLAKE3 content hashing for file change detection, `file_manifest` CozoDB relation storing per-file hashes grouped by language unit, `copy_nodes`/`copy_edges` store methods for carrying forward unchanged data, unit-level skip with copy-then-upsert strategy, `svt analyze --incremental` CLI flag with auto-detection of latest analysis version as previous.

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

**Not yet done (deferred):** ~~Error boundary components with retry~~ (resolved in M23), ~~diff view overlay in graph~~ (resolved in M16), ~~arrow-key graph traversal~~ (resolved in M23).

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

**Not yet done (deferred):** ~~Plugin manifest format (`svt-plugin.toml`), `svt plugin install|remove` commands~~ — resolved in M22. Plugin sandboxing remains as future work. ~~`LanguageOrchestrator` support in plugin API~~ — resolved in M18.

### Milestone 18: Plugin Analyzer Support — COMPLETE

**Goal:** Allow external plugins to contribute language analyzers via the `SvtPlugin` trait.

**Delivered:**
- `LanguageDescriptor` struct in svt-core: language_id, manifest_files, source_extensions, source_dirs, exclude_dirs (WASM-compatible, no platform dependencies)
- `LanguageParser` trait in svt-core: `parse()`, `emit_structural_items()`, `post_process()` with default impls for optional hooks
- `ParseResult` type in svt-core: items, relations, warnings — shared between core and analyzer
- `SvtPlugin::language_parsers()` method returning `Vec<(LanguageDescriptor, Box<dyn LanguageParser>)>` (empty default)
- `DescriptorOrchestrator` in svt-analyzer: generic adapter wrapping any descriptor+parser pair into a `LanguageOrchestrator`
- Go, Python, TypeScript orchestrators refactored to descriptor+parser pattern (factory functions delegating to `DescriptorOrchestrator`)
- `RustAnalyzer` implements `LanguageParser` trait (Rust keeps custom orchestrator for workspace root emission)
- `analyze_project_with_registry()` in svt-analyzer accepts custom `OrchestratorRegistry`
- `PluginLoader::register_language_parsers()` wires plugin contributions into analysis pipeline
- `svt plugin list` shows language parsers with manifest files and source extensions
- Re-exported analysis types (`AnalysisItem`, `AnalysisRelation`, `AnalysisWarning`) from svt-core through svt-analyzer (zero-cost, same types)
- 10 new DescriptorOrchestrator tests, existing 370+ tests all pass
- **Result: 382 Rust tests + 22 vitest tests = 404 total**
- **Dog-food: 878 nodes, 907 edges, conformance 12/12 passed**

### Milestone 19: Store Persistence & Management — COMPLETE

**Goal:** Add persistent storage to the server, schema migrations, store management CLI, and store info API.

**Delivered:**
- Schema version + migration framework: `CURRENT_SCHEMA_VERSION`, `metadata` relation, `schema_version()`, `set_schema_version()`, `migrate()` with forward-only versioning
- `SchemaMismatch` and `CorruptStore` error variants in `StoreError`
- `store_info()` method on `GraphStore` trait with `StoreInfo` and `SnapshotSummary` types (per-snapshot node/edge counts)
- `svt store info` — formatted table with version, kind, nodes, edges, commit, timestamp
- `svt store compact [--keep <versions>]` — default keeps latest design + latest analysis
- `svt store reset [--force]` — delete and recreate store
- Server `--store <PATH>` flag for persistent SQLite-backed CozoDB storage
- Server startup relaxed: `--store` alone is valid if store has existing data
- `GET /api/store/info` endpoint returning JSON store metadata
- 16 new tests (5 schema migration, 3 store_info, 6 CLI store commands, 2 server store endpoint)
- **Result: 398 Rust tests + 22 vitest tests = 420 total**
- **Dog-food: 852 nodes, 863 edges, conformance 12/12 passed**

### Milestone 20: Incremental Analysis — COMPLETE

**Goal:** Skip re-analysis of unchanged language units by tracking file content hashes per snapshot.

**Delivered:**
- `FileManifestEntry` type in svt-core model (path, hash, unit_name, language)
- `file_manifest` CozoDB relation in `init_schema()` with `(path, version)` key
- `add_file_manifest()`, `get_file_manifest()`, `copy_nodes()`, `copy_edges()` on `GraphStore` trait
- `compact()` updated to clean up file_manifest entries
- BLAKE3 content hashing in `crates/analyzer/src/hashing.rs` (`hash_file`, `build_manifest`, `changed_units`)
- `analyze_project_incremental()` and `analyze_project_incremental_with_registry()` pipeline
- Unit-level skip with copy-then-upsert: copy ALL nodes/edges from previous, then upsert changed units on top
- `AnalysisSummary` extended with `incremental`, `units_skipped`, `units_reanalyzed`, `nodes_copied`, `edges_copied`
- `svt analyze --incremental` CLI flag with auto-detection of latest analysis version
- Falls back to full analysis when no previous version or manifest exists (stores manifest for next run)
- 33 new tests: 10 file_manifest store tests, 13 hashing unit/proptests, 3 incremental pipeline tests, 3 CLI tests, 4 integration tests
- **Result: 431 Rust tests + 22 vitest tests = 453 total**

**Known limitation (v1):** If a language unit is entirely removed between runs, its ghost nodes remain (copied from previous, never overwritten). Periodic full analysis cleans this up.

### Milestone 21: Analysis Depth — COMPLETE

**Goal:** Improve method call resolution depth through crate-level dependency extraction, Self::/Type:: associated call resolution, and heuristic local variable type inference.

**Delivered:**
- Crate-level `Depends` edges from Cargo metadata: `CrateInfo.workspace_dependencies` populated from `cargo metadata`, `RustOrchestrator.post_process()` emits `Depends` relations between workspace crates
- `resolve_scoped_call()` function: rewrites `Self::method()` to `Type::method()` using impl context, prepends module context for local `Type::method()` calls
- `build_local_type_map()` with `collect_let_declarations()`: walks function bodies for `let` bindings, extracts type from explicit annotations (`let x: Foo`), constructor calls (`let x = Foo::new()`), and struct expressions (`let x = Foo { ... }`)
- `extract_param_types()`: extracts function parameter name→type mappings with `&`/`&mut` reference stripping
- Modified `visit_call_expressions()` to use local type map for receiver type lookup on `x.method()` calls
- Method call resolution statistics: `method_calls_resolved`/`method_calls_unresolved` counters threaded through entire analysis pipeline
- CLI output shows resolution stats: "method calls: N resolved, M unresolved (of T total)"
- 17 new tests (12 unit tests in rust.rs, 2 discovery/orchestrator tests, 3 integration tests)
- **Result: 448 Rust tests + 22 vitest tests = 470 total**
- **Dog-food: 954 nodes, 1076 edges, 468/3997 method calls resolved (11.7%), conformance 12/12 passed**

**Known limitations:**
- No chained call resolution (`x.foo().bar()` — receiver of `.bar()` is a call expression)
- No field access resolution (`self.field.method()`)
- No trait object/generic resolution (`Box<dyn Foo>`, `fn f<T: Foo>(x: T)`)
- No closure parameter types (`items.iter().map(|x| x.foo())`)
- Import aliasing not handled (`use other::Foo; Foo::new()`)
- Only workspace-internal Cargo dependencies (external crates not represented)

### Milestone 22: Plugin Ecosystem — COMPLETE

**Goal:** Add plugin manifest format, install/remove/info CLI commands, manifest-aware loading, and plugin authoring documentation.

**Delivered:**
- `PluginManifest` struct with TOML parsing/validation/serialization (`svt-plugin.toml` format)
- `PluginMetadata` (name, version, description, authors, license, api_version, library) and `PluginContributions` (constraint_kinds, export_formats, language_ids)
- `ManifestError` enum with `thiserror` for structured error reporting
- Platform-aware library filename derivation (`lib<name>.dylib`/`.so`/`.dll`)
- `LoadedPlugin` wrapper pairing plugin instance with path, manifest, and `PluginSource` (CliFlag/ProjectLocal/UserGlobal)
- Sidecar manifest discovery: `<stem>.svt-plugin.toml` then `svt-plugin.toml` in library directory
- `svt plugin install <source> [--global] [--force]` — copies library + manifest to plugins directory
- `svt plugin remove <name> [--global]` — removes plugin by name
- `svt plugin info <path>` — displays manifest metadata with API compatibility check
- Enhanced `svt plugin list` showing source label, manifest description, and contribution details
- Comprehensive plugin authoring guide (`docs/plugin-authoring.md`): Quick Start, trait implementation, contributing types, manifest format, building/installing, testing, API reference, troubleshooting
- 36 new tests (12 manifest, 4 plugin loader, 15 plugin commands, 5 CLI integration)
- **Result: 484 Rust tests + 22 vitest tests = 506 total**
- **Dog-food: conformance 12/12 passed**

**Not yet done (deferred):** Remote plugin registry, plugin dependencies, plugin hot-reloading, plugin configuration/settings, plugin sandboxing.

### Milestone 23: Web UI Enhancements — COMPLETE

**Goal:** Add error boundaries with retry, arrow-key graph traversal, and a filtering sidebar.

**Delivered:**
- `ErrorBoundary.svelte` using Svelte 5's `<svelte:boundary>` with `{#snippet failed}` and `{#key retryKey}` for full remount on retry
- Error boundaries wrap GraphView, NodeDetail, and ConformanceReport sections
- Arrow-key graph traversal: Up=parent, Down=first child, Left/Right=prev/next sibling in containment hierarchy
- Pre-computed `TraversalIndex` with O(1) lookups (parentMap, childrenMap, siblingsMap sorted by label)
- `FilterSidebar.svelte`: collapsible left sidebar with checkbox filters for node kind, edge kind, sub-kind, and language
- `filter.svelte.ts` reactive store with `populateFromGraph()`, `resetAll()`, `hasActiveFilters`
- Filters applied client-side via Cytoscape `startBatch()`/`endBatch()` show/hide
- Sidebar state persisted to localStorage, toggled via toolbar button or `g` key
- `*` indicator on filter button when filters are active
- 15 traversal tests + 12 filter logic tests = 27 new vitest tests
- **Result: 484 Rust tests + 49 vitest tests = 533 total**

**Not yet done (deferred):** Provenance filtering (requires adding provenance to Cytoscape graph endpoint), URL hash persistence of filter state, filter count badges, component-level tests with @testing-library/svelte.

### Milestone 24: Multi-Tenancy (Project-Scoped Store) — COMPLETE

**Goal:** Enable a single server instance to host multiple independent projects, each with its own version numbering, snapshots, and data. Add a CLI `push` command for uploading analysis results to a remote server.

**Delivered:**
- `Project` model type with `id`, `name`, `created_at`, `description`, `metadata` fields
- `validate_project_id()` — lowercase alphanumeric + hyphens/underscores, 1-128 chars
- `ProjectNotFound`, `DuplicateProject`, `InvalidProjectId` error variants
- `GraphStore` trait extended with `create_project()`, `list_projects()`, `get_project()`, `project_exists()`
- `create_snapshot()`, `list_snapshots()`, `latest_version()`, `compact()` now project-scoped
- CozoDB schema v2 migration: `projects` + `snapshot_projects` companion relations
- Automatic migration of existing v1 stores: all snapshots tagged with `"default"` project
- Server: `RwLock<CozoStore>` for safe concurrent access, `read_store()`/`write_store()` helpers
- Server: project CRUD routes (`GET/POST /api/projects`, `GET /api/projects/{project}`)
- Server: project-scoped routes (`/api/projects/{project}/snapshots/...`, etc.)
- Server: push endpoint (`POST /api/projects/{project}/push`) for CLI uploads
- Server: legacy routes default to `"default"` project for backwards compatibility
- Server: `--project` flag for startup project scoping
- CLI: `svt push --server URL [--project NAME]` command with project name auto-derivation from git remote
- CLI: `--project` global flag threaded through all commands
- WASM: `load_snapshot()` gains optional `project_id` parameter, default project seeded on init
- Web UI: `ProjectSelector.svelte` dropdown (hidden when single project)
- Web UI: project-scoped API functions, `projects`/`selectedProject` in graph store
- Web UI: `p=<project>` hash parameter in URL routing
- 85 new tests across all crates (569 Rust + 194 vitest)
- **Result: 569 Rust tests + 194 vitest tests = 763 total**

## Analyzer Feature Parity

The Rust analyzer is the most complete. Other analyzers need to reach parity across these dimensions:

| Feature | Rust | TypeScript | Go | Python |
|---------|:----:|:----------:|:--:|:------:|
| **Structural** | | | | |
| Functions | Y | Y | Y | Y |
| Methods (class/impl) | Y | Y (members) | Y (receiver) | Y |
| Structs/Classes | Y | Y | Y | Y |
| Enums/Variants | Y | Y (members) | — | — |
| Traits/Interfaces | Y | Y (members) | Y | — |
| Module hierarchy | Y (file + `mod`) | Y (directory) | Y (directory) | Y (file + directory) |
| **Edges** | | | | |
| Depends (imports) | Y | Y (relative only) | Y (raw paths) | Y (relative + absolute) |
| Calls (call graph) | Y | Y | Y (import-aware) | Y (self-aware) |
| Implements | Y | Y | — | — |
| Extends | — | Y | — | — |
| Exports (re-exports) | Y | — | — | — |
| Cross-pkg deps | Y (Cargo metadata) | Y (package.json) | Y (go.mod) | Y (pyproject.toml) |
| **Resolution** | | | | |
| Import path resolution | Y | Y (post-process) | — | Y (relative imports) |
| Method call resolution | Y (type inference) | Y (this) | Y (import aliases) | Y (self) |
| Use/import aliases | Y | — | Y | — |
| **Metadata** | | | | |
| LOC | Y | Y | Y | Y |
| Test detection/tagging | Y | Y | Y | Y |
| **Post-Processing** | | | | |
| Qualified name rewriting | Y | Y | Y | Y |
| Type registry | Y | — | — | — |
| Structural item emission | Y | Y | Y | Y |

## Roadmap (Post-M29)

Priority-ordered next milestones:

| # | Milestone | Description | Key Challenge |
|---|-----------|-------------|---------------|
| **M25** | Test Detection (All Languages) | **COMPLETE** | — |
| **M26** | Module Hierarchy & Post-Processing (Go + Python) | **COMPLETE** | — |
| **M27** | TypeScript Structural Depth | **COMPLETE** | — |
| **M28** | Call Graph Analysis (TypeScript + Go + Python) | **COMPLETE** | — |
| **M29** | Cross-Package Dependency Extraction | **COMPLETE** | — |
| **M30** | Java Analyzer | New language: tree-sitter-java with full structural extraction and call graph | Maven/Gradle project discovery, class hierarchy, annotation processing |

### M25: Test Detection (All Languages) — COMPLETE

**Goal:** Tag test code across all languages so visualizations can dim/filter test nodes (as they already can for Rust).

**Delivered:**
- **TypeScript:** Files matching `*.test.ts`/`*.spec.ts`/`__tests__/*` tagged with `test`; `describe`/`it`/`test` blocks detected
- **Go:** `_test.go` files included in analysis (previously excluded) with all items tagged `test`; `Test*`/`Benchmark*`/`Example*`/`Fuzz*` functions tagged
- **Python:** `test_*.py`/`*_test.py`/`conftest.py` files included (previously excluded) with `test` tags; `test_*` functions and `Test*` classes tagged
- `is_go_test_file()`, `go_test_tags()`, `is_python_test_file()`, `is_typescript_test_file()` helper functions

### M26: Module Hierarchy & Post-Processing (Go + Python) — COMPLETE

**Goal:** Both Go and Python lack synthetic module hierarchy nodes and import resolution post-processing.

**Delivered:**
- **Go:** Directory-level `Component/module` nodes via `emit_go_module_items()`; Go packages = directories (no file-level nodes); method reparenting preserving receiver types in `post_process()`
- **Python:** File + directory `Component/module` nodes via `emit_python_module_items()`; `__init__.py` items belong to parent package; relative import resolution via `.module@filepath` encoding and `post_process()` rewriting
- `go_dir_to_module_qn()`, `python_file_to_module_qn()`, `resolve_python_relative_import()` helpers

### M27: TypeScript Structural Depth — COMPLETE

**Goal:** TypeScript currently only extracts exported declarations. Bring it to structural parity with Rust's type-level extraction.

**Delivered:**
- Class members: methods, properties, constructors extracted as child nodes
- Interface members extracted as child nodes
- Enum members extracted as variant nodes (matching Rust's pattern)
- `Extends` edges for class inheritance (`class Foo extends Bar`)
- `Implements` edges for class-interface relationships (`class Foo implements Bar`)
- Non-exported items now extracted (functions, classes, interfaces, enums, type aliases)
- `extract_class_members()`, `extract_interface_members()`, `extract_enum_members()`, `extract_heritage_clauses()` functions

### M28: Call Graph Analysis (TS + Go + Python) — COMPLETE

**Goal:** Only Rust currently extracts `Calls` edges. Add call graph analysis to the other three languages.

**Delivered:**
- **TypeScript:** `visit_ts_call_expressions()` — handles `identifier` (simple calls) and `member_expression` (`this.method()` resolution via class context)
- **Go:** `visit_go_call_expressions()` with `collect_go_import_aliases()` — resolves `pkg.Func()` calls via import alias map to full qualified names
- **Python:** `visit_py_call_expressions()` — resolves `self.method()` calls via class context; handles `identifier` and `attribute` call patterns
- Recursive AST walking of function/method bodies across all three languages

### M29: Cross-Package Dependency Extraction — COMPLETE

**Goal:** Rust extracts workspace-internal dependencies from Cargo metadata. Other languages should extract equivalent information from their build tools.

**Delivered:**
- `workspace_dependencies: Vec<String>` field added to `LanguageUnit`
- **TypeScript:** `parse_ts_dependencies()` — extracts from `dependencies` and `devDependencies` in `package.json`
- **Go:** `parse_go_requires()` — parses `require (...)` blocks and single `require` lines from `go.mod`
- **Python:** `parse_pyproject_dependencies()` — extracts from `[project].dependencies` in `pyproject.toml` with name normalization (`-` → `_`)
- `resolve_workspace_dependencies()` in `DescriptorOrchestrator` — builds known-unit sets, filters to workspace-internal deps
- `Depends` edges emitted during `post_process()` for each resolved workspace dependency

### M30: Java Analyzer

**Goal:** New language analyzer following established patterns from M25–M29. Should launch with feature parity matching the enhanced TypeScript/Go/Python analyzers.

**Scope:**
- tree-sitter-java grammar integration
- Maven (`pom.xml`) and Gradle (`build.gradle`/`build.gradle.kts`) project discovery
- Class, interface, enum, annotation, method, field extraction
- Package hierarchy from directory structure (`src/main/java/...`)
- Import resolution and `Extends`/`Implements` edges
- Call graph analysis (method calls with type resolution)
- Test detection: JUnit `@Test`/`@ParameterizedTest`, TestNG `@Test`, files in `src/test/java/`
- Cross-module dependencies from Maven/Gradle metadata

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
| `2026-02-20-plugin-analyzer-support-design.md` | M18 design (plugin analyzer support) |
| `2026-02-20-plugin-analyzer-support-implementation.md` | M18 implementation plan (COMPLETE) |
