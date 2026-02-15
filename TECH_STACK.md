# Software Visualizer Tool — Tech Stack

## Decisions

| Layer | Choice | Rationale |
|-------|--------|-----------|
| **Backend language** | Rust | Performance, single binary, tree-sitter native, WASM compilation |
| **Graph store** | CozoDB (embedded) | Rust-native, WASM-compatible, Datalog query language maps naturally to graph traversal and conformance rules |
| **Shared core (WASM)** | Rust → wasm-bindgen | Same conformance/validation logic in CLI, API, and browser |
| **Frontend framework** | Svelte | Lightweight rendering layer, minimal overhead around graph visualization |
| **Graph visualization** | Cytoscape.js | Multi-layout, compound nodes for drill-down, conditional styling for conformance, canvas-based performance |
| **API service** | Axum | Rust-native async web framework |
| **Export formats** | SVG/PNG, Markdown+Mermaid, JSON | Interoperability principle — pluggable, core set ships built-in |

## Architecture

```
crates/
  core/        — data model, CozoDB graph store, validation, conformance (compiles to WASM)
  analyzer/    — tree-sitter code analysis, discovery mode
  cli/         — CLI entry point, export (Mermaid/SVG/PNG)
  server/      — Axum API service, serves web UI

web/           — Svelte frontend + Cytoscape.js + WASM core
```

## Dependency Flow

```
cli ──→ analyzer ──→ core
server ──→ analyzer ──→ core
web (via WASM) ──────→ core
```

Dependencies flow inward. Core has no dependency on CLI, server, or analyzer.
