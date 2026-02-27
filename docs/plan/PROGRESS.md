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

**Current state:** 484 Rust tests + 49 vitest tests = 533 total. All passing. clippy/fmt/audit clean. CI pipeline operational.

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
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
svt-server --store .svt/store            # Serve with persistent storage (data survives restart)
svt-server --store .svt/store --design design/architecture.yaml
                                         # Persistent store + fresh design import at startup
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), conformance overlay, diff view overlay, error boundaries with retry, arrow-key graph traversal, and a filtering sidebar for node/edge/sub-kind/language filtering. With WASM loaded, node detail lookups and search run entirely in the browser — zero API round-trips after initial snapshot load.

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

### Analyzer Feature Parity — OPEN
- The Rust analyzer is significantly more capable than the TypeScript, Go, and Python analyzers. Key gaps: no test detection (TS/Go/Python), no module hierarchy (Go/Python), no call graph (TS/Go/Python), no class member extraction (TS), no cross-package dependency extraction (TS/Go/Python). See parity matrix and M24–M29 roadmap below.

### Git Integration — RESOLVED (M13 + M16)
- ~~`analyze_project()` accepts an optional `commit_ref` but there is no automatic git-aware snapshot creation or change detection.~~ `svt analyze` now auto-detects git HEAD when `--commit-ref` is not provided. Change detection between snapshots is available via `svt diff`. Web UI diff view overlay added in M16.

### Dynamic Plugin Loading — RESOLVED (M17 + M22)
- ~~Plugin registries exist with `.register()` API but all plugins are compiled in. No external plugin discovery, no dynamic loading, no plugin manifest format.~~ Resolved: `SvtPlugin` trait + `declare_plugin!` macro, `PluginLoader` with `libloading`, `--plugin` flag, `svt plugin list`, 3-tier discovery (CLI/project-local/user-global). ~~Plugin manifest format (`svt-plugin.toml`) and install/remove commands remain as future work.~~ Resolved in M22: `svt-plugin.toml` manifest format, `svt plugin install|remove|info` commands, manifest-aware loading with source tracking, plugin authoring documentation.

### Plugin Analyzer Support — RESOLVED (M18)
- ~~`LanguageOrchestrator` lives in svt-analyzer; plugins depend on svt-core only. External plugins cannot contribute language analyzers.~~ Resolved: `LanguageDescriptor` struct + `LanguageParser` trait in svt-core (WASM-compatible), `DescriptorOrchestrator` adapter in svt-analyzer, `SvtPlugin::language_parsers()` method. Go, Python, TypeScript refactored to descriptor+parser pattern. Plugin language contributions wired into CLI analysis pipeline via `analyze_project_with_registry()`.

### Store Persistence — RESOLVED (M19)
- ~~The server always uses `CozoStore::new_in_memory()`, losing all data on restart. No schema migration system, no store management CLI commands, no way to inspect or compact the store.~~ Resolved: Schema version + migration framework (`CURRENT_SCHEMA_VERSION`, `schema_version()`, `migrate()`), `store_info()` with per-snapshot node/edge counts, `svt store info|compact|reset` CLI commands, `--store` flag for server persistent CozoDB storage, `GET /api/store/info` endpoint.

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

## Analyzer Feature Parity

The Rust analyzer is the most complete. Other analyzers need to reach parity across these dimensions:

| Feature | Rust | TypeScript | Go | Python |
|---------|:----:|:----------:|:--:|:------:|
| **Structural** | | | | |
| Functions | Y | Y (exports only) | Y | Y |
| Methods (class/impl) | Y | — | Y (receiver) | Y |
| Structs/Classes | Y | Y (exports only) | Y | Y |
| Enums/Variants | Y | — | — | — |
| Traits/Interfaces | Y | Y (exports only) | Y | — |
| Module hierarchy | Y (file + `mod`) | Y (directory) | — | — |
| **Edges** | | | | |
| Depends (imports) | Y | Y (relative only) | Y (raw paths) | Y (basic) |
| Calls (call graph) | Y | — | — | — |
| Implements | Y | — | — | — |
| Extends | — | — | — | — |
| Exports (re-exports) | Y | — | — | — |
| Cross-pkg deps | Y (Cargo metadata) | — | — | — |
| **Resolution** | | | | |
| Import path resolution | Y | Y (post-process) | — | — |
| Method call resolution | Y (type inference) | — | — | — |
| Use/import aliases | Y | — | — | — |
| **Metadata** | | | | |
| LOC | Y | Y | Y | Y |
| Test detection/tagging | Y | — | — | — |
| **Post-Processing** | | | | |
| Qualified name rewriting | Y | Y | — | — |
| Type registry | Y | — | — | — |
| Structural item emission | Y | Y | — | — |

## Roadmap (Post-M23)

Priority-ordered next milestones:

| # | Milestone | Description | Key Challenge |
|---|-----------|-------------|---------------|
| **M24** | Test Detection (All Languages) | Tag test code in TypeScript, Go, and Python analyzers | Including previously-excluded test files with proper tagging |
| **M25** | Module Hierarchy & Post-Processing (Go + Python) | Emit synthetic module nodes, resolve import paths | Go package hierarchy, Python `__init__.py` detection, relative imports |
| **M26** | TypeScript Structural Depth | Class methods/properties, extends/implements edges, enum members | Balancing exports-only vs full extraction, interface member extraction |
| **M27** | Call Graph Analysis (TypeScript + Go + Python) | Extract `Calls` edges from function/method bodies in all non-Rust languages | Type inference for method call receivers, import-resolved call targets |
| **M28** | Cross-Package Dependency Extraction | Extract workspace-internal dependencies from build tool metadata | npm/yarn/pnpm workspaces, `go.mod` require directives, `pyproject.toml` deps |
| **M29** | Java Analyzer | New language: tree-sitter-java with full structural extraction and call graph | Maven/Gradle project discovery, class hierarchy, annotation processing |

### M24: Test Detection (All Languages)

**Goal:** Tag test code across all languages so visualizations can dim/filter test nodes (as they already can for Rust).

**Scope:**
- **TypeScript:** Detect `describe`/`it`/`test` from vitest/jest; tag files matching `*.test.ts`/`*.spec.ts`/`__tests__/*`
- **Go:** Include `_test.go` files (currently excluded entirely) but tag all items with `test`; detect `func Test*`/`func Bench*`
- **Python:** Include `test_*.py`/`*_test.py`/`conftest.py` (currently excluded entirely) but tag with `test`; detect `test_*` functions, `unittest.TestCase` subclasses

**Key challenge:** Go and Python currently skip test files at the discovery level. Need to include them in analysis but tag them, matching the Rust pattern where `#[cfg(test)]` modules are analyzed but tagged.

### M25: Module Hierarchy & Post-Processing (Go + Python)

**Goal:** Both Go and Python lack synthetic module hierarchy nodes and import resolution post-processing. This is foundational for deeper analysis.

**Scope:**
- **Go:** Emit `Component` nodes for each Go package from the package directory structure; build parent-child hierarchy from package import paths; resolve raw import paths to qualified names in post-processing
- **Python:** Emit module hierarchy from directory structure (`__init__.py` as package marker); resolve relative imports (`from . import foo`, `from ..sibling import bar`); add post-processing pass for item reparenting

**Key challenge:** Go packages are identified by import path (not directory name) and can contain multiple files. Python has complex relative import semantics with `__init__.py` as both a package marker and code file.

### M26: TypeScript Structural Depth

**Goal:** TypeScript currently only extracts exported declarations. Bring it to structural parity with Rust's type-level extraction.

**Scope:**
- Extract class methods, properties, and constructors as `Unit` child nodes
- Extract interface members
- Emit `Extends` edges for class inheritance (`class Foo extends Bar`)
- Emit `Implements` edges for class-interface relationships (`class Foo implements Bar`)
- Extract enum members as variant nodes (matching Rust's enum variant extraction)
- Consider extracting non-exported items (opt-in flag) for internal architecture analysis

**Key challenge:** Balancing the current exports-only design (keeps graph small) with the need for structural depth. May need an analysis depth option.

### M27: Call Graph Analysis (TypeScript + Go + Python)

**Goal:** Only Rust currently extracts `Calls` edges. Add call graph analysis to the other three languages.

**Scope:**
- **TypeScript:** Function/method calls within bodies; import-resolved call targets; `new ClassName()` as calls
- **Go:** Function calls; method calls with receiver type resolution (already has receiver type extraction); interface satisfaction detection → `Implements` edges
- **Python:** Function calls; method calls; `ClassName()` instantiation as calls; decorator invocations

**Key challenge:** Type inference for method call receivers — Go has receiver types on declarations but needs them at call sites; TypeScript/Python need heuristic type inference similar to Rust's `build_local_type_map()`. Could be split into per-language sub-milestones if scope is too large.

### M28: Cross-Package Dependency Extraction

**Goal:** Rust extracts workspace-internal dependencies from Cargo metadata. Other languages should extract equivalent information from their build tools.

**Scope:**
- **TypeScript:** Extract workspace dependencies from `package.json` (npm/yarn/pnpm workspaces); detect monorepo structure
- **Go:** Extract module dependencies from `go.mod` `require` directives; filter to workspace-internal modules
- **Python:** Extract dependencies from `pyproject.toml` `[project.dependencies]`; detect monorepo-internal packages

**Key challenge:** Each ecosystem has different workspace/monorepo conventions. Need to reliably distinguish internal vs external dependencies.

### M29: Java Analyzer

**Goal:** New language analyzer following established patterns from M24–M28. Should launch with feature parity matching the enhanced TypeScript/Go/Python analyzers.

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
