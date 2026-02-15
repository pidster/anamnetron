# ADR-002: CozoDB as Primary Graph Store

## Status

Accepted

## Context

The data model is a directed, typed property graph. The tool needs to perform graph traversal, pathfinding, cycle detection, subgraph extraction, and conformance rule evaluation. The store must be embedded (no external service), work offline, and ideally compile to WASM.

## Decision

Use CozoDB as the primary graph store, behind an abstract `GraphStore` trait. SurrealDB is the designated fallback if CozoDB struggles at scale.

## Alternatives Considered

- **SurrealDB** — more mature at scale, multi-model (graph + document), embedded mode available. SurrQL is SQL-like, less natural for recursive graph queries than Datalog. WASM support less proven. Designated as fallback.
- **petgraph** — in-memory Rust graph library. Excellent for algorithms but no persistence, no query language, pushes all storage logic onto application code.
- **Neo4j** — full graph database with Cypher. Requires external JVM process — violates single-binary usability principle.
- **SQLite + recursive CTEs** — ubiquitous and embedded, but graph queries are awkward and verbose.

## Consequences

- Datalog query language maps naturally to conformance rules — user-defined rules may fall out of the data store choice rather than requiring a separate rules engine.
- CozoDB compiles to WASM — browser can run subgraph queries directly.
- `GraphStore` trait abstraction means the backend can be swapped without changing the data model or application logic.
- Performance must be benchmarked early against medium (~150K edges) and large (~1.5M edges) datasets.
- Different backends may be used for different deployment contexts: CozoDB in browser (WASM), potentially SurrealDB for server at scale.
