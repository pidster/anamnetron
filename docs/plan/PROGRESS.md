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

**Current state:** 229 Rust tests + 5 vitest tests = 234 total. All passing. clippy/fmt/audit clean.

## What's Working Now

```
svt import design/architecture.yaml     # Load a design model
svt check                                # Conformance check (design-only)
svt analyze --project .                  # Analyze Rust project with tree-sitter
svt check --analysis                     # Compare design vs analysis
svt export --format mermaid              # Export as Mermaid flowchart
svt export --format json                 # Export as interchange JSON
svt export --format mermaid -o arch.mmd  # Export to file
svt-server --design design/architecture.yaml --project .
                                         # Serve API + web UI at http://localhost:3000
```

The web UI renders the architecture graph with compound nodes, click-to-inspect node details, search, layout switching (force-directed / hierarchical), and conformance overlay.

All 10 constraints in `design/architecture.yaml` are now fully evaluated — zero `NotEvaluable`.

## Not Yet Built (from design/architecture.yaml & PRINCIPLES.md)

### Analyzers
- **TypeScript analyzer** — tree-sitter-typescript (architecture.yaml: `/svt/analyzer/languages/typescript`)

### WASM
- **WASM bridge** — svt-core compiled to wasm-bindgen for browser-side queries (architecture.yaml: `/svt/web/wasm`)

### Infrastructure
- **Plugin API** — Extensibility for language analyzers, constraint types, export formats (PRINCIPLES.md: Extensibility)
- **CI integration** — GitHub Actions workflow, conformance as CI gate (PRINCIPLES.md: Quality)

## Suggested Next Milestones

### Milestone 7: TypeScript Analyzer

**Goal:** Add a second language analyzer, proving the multi-language architecture works.

**Scope:**
- tree-sitter-typescript integration
- Package/module/class/function/interface extraction
- Import/export edge detection
- Canonical path mapping for TypeScript projects
- Dog-food: analyze the `web/` Svelte/TS code alongside the Rust crates

**Why next:** The analyzer architecture was designed for multiple languages. TypeScript is the natural second language given the web frontend is TypeScript. Dog-fooding on the project's own frontend code validates the multi-language story.

### Milestone 8: WASM Bridge

**Goal:** Compile svt-core to WASM for browser-side graph queries without server round-trips.

**Scope:**
- wasm-bindgen bindings for GraphStore operations
- Browser-side subgraph filtering and search
- Reduce API round-trips for node detail lookups
- Verify core crate has no platform-specific dependencies

### Milestone 9: CI + Plugin Foundations

**Goal:** GitHub Actions CI pipeline and initial plugin API surface.

**Scope:**
- GitHub Actions workflow (build, test, lint, fmt, audit, cross-platform)
- Conformance check as CI gate
- Plugin trait definitions for analyzers and constraint types
- Plugin discovery and loading

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
