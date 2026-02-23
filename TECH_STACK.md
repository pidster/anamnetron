# Software Visualizer Tool — Tech Stack

## Decisions

| Layer | Choice | Rationale |
|-------|--------|-----------|
| **Backend language** | Rust | Performance, single binary, tree-sitter native, WASM compilation |
| **Graph store** | CozoDB (primary), SurrealDB (fallback) | CozoDB: Rust-native, WASM-compatible, Datalog maps naturally to conformance rules. SurrealDB: proven at scale, fallback if CozoDB struggles with large datasets |
| **Shared core (WASM)** | Rust → wasm-bindgen | Same conformance/validation logic in CLI, API, and browser |
| **Frontend framework** | Svelte | Lightweight rendering layer, minimal overhead around graph visualization |
| **Interactive graph** | Cytoscape.js | Multi-layout (ELK, Dagre, fCOSE), compound nodes, conformance/diff overlays, minimap, canvas-based performance |
| **Diagram generation** | Mermaid | Flowchart, data flow, sequence, C4 diagrams; text-based, exportable, primary visualization view |
| **Analytical charts** | D3.js (planned) | Chord diagrams, treemaps, sunburst charts — visualization types that require custom layout algorithms |
| **API service** | Axum | Rust-native async web framework |
| **Export formats** | SVG/PNG, Markdown+Mermaid, JSON | Interoperability principle — pluggable, core set ships built-in |

## Architecture

```
crates/
  core/        — data model, graph store trait, validation, conformance (compiles to WASM)
  analyzer/    — tree-sitter code analysis, discovery mode
  cli/         — CLI entry point, export (Mermaid/SVG/PNG)
  server/      — Axum API service, serves web UI

web/           — Svelte frontend + Mermaid + Cytoscape.js + D3.js (planned) + WASM core
```

## Dependency Flow

```
cli ──→ analyzer ──→ core
server ──→ analyzer ──→ core
web (via WASM) ──────→ core
```

Dependencies flow inward. Core has no dependency on CLI, server, or analyzer.

## Graph Store Strategy

The graph store is abstracted behind a `GraphStore` trait in `svt-core`. This decouples the data model from the storage engine and allows different backends for different contexts.

**Primary: CozoDB**
- Datalog query language is a natural fit for recursive graph traversal and conformance rule expression
- Compiles to WASM — suitable for browser-side subgraph queries
- Embedded, no external process — aligns with single-binary usability principle

**Fallback: SurrealDB**
- More mature at scale — designed for production workloads with large datasets
- Embedded mode available (Rust-native), can also run as a service
- SurrQL is SQL-like, less natural for recursive graph queries but more familiar

**Deployment-specific selection:**
- **Browser (WASM):** CozoDB — proven WASM target, datasets are smaller (viewing subgraphs)
- **CLI / Server:** CozoDB initially, SurrealDB if benchmarks show scaling issues

**Performance mitigations (store-agnostic):**
- Index edges on `source`, `target`, and `kind`
- Materialise frequently-queried aggregations (e.g., component-level dependency summaries)
- Partition by provenance where beneficial
- Incremental analysis — diff changed files, update only affected subgraphs
- Push filtering into the query engine, not application code
- Benchmark early against medium (~150K edges) and large (~1.5M edges) datasets
