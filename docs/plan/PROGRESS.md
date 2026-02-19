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

**Current state:** 277 Rust tests + 5 vitest tests = 282 total. All passing. clippy/fmt/audit clean. CI pipeline operational.

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

All 10 constraints in `design/architecture.yaml` are now fully evaluated — zero `NotEvaluable`.

## Not Yet Built (from design/architecture.yaml & PRINCIPLES.md)

### Infrastructure
- ~~**CI integration** — GitHub Actions workflow, conformance as CI gate (PRINCIPLES.md: Quality)~~ — **Done (Milestone 9)**
- **Plugin API** — Extensibility for language analyzers, constraint types, export formats (PRINCIPLES.md: Extensibility)

## Suggested Next Milestones

### Milestone 10: Plugin Foundations (in progress)

**Goal:** Trait-based registries for constraint evaluators, export formats, and language analyzers.

**Scope:**
- `ConstraintEvaluator`, `ExportFormat`, and extended `LanguageAnalyzer` traits
- `ConstraintRegistry`, `ExportRegistry`, `AnalyzerRegistry` structs with `with_defaults()` + `.register()`
- Refactor existing built-ins into trait implementations
- Replace hardcoded `match` dispatch with registry lookups in CLI, server, and conformance engine
- `GraphStore` as `dyn` for trait object compatibility

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
| `2026-02-19-milestone-10-design.md` | M10 design |
