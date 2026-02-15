# ADR-006: Graph Store Trait Abstraction

## Status

Accepted

## Context

The choice of graph store (CozoDB) is based on current fit but may not scale to large codebases. Different deployment contexts (browser via WASM, CLI, server) may benefit from different backends. Coupling the application to a specific database would make future migration costly.

## Decision

Define a `GraphStore` trait in `svt-core` that abstracts all graph operations. Implement CozoDB as the first backend. The trait covers: write (including batch), read, traversal, aggregation, conformance evaluation, and version management.

## Alternatives Considered

- **Direct CozoDB API usage** — simpler initially, but locks the entire codebase to CozoDB. Migration requires touching every call site.
- **Generic query interface** — expose raw Datalog/SQL. Too leaky — application code would be tied to the query language.

## Consequences

- Backend can be swapped without changing application logic.
- Different backends can serve different contexts (CozoDB for WASM/browser, SurrealDB for server at scale).
- The trait must be expressive enough to let backends optimise (e.g., materialised views for aggregation), not just a thin CRUD layer.
- No raw query exposure in the trait — typed methods only. Raw access may be added later if needed.
- Version is explicit in every trait method — no implicit "current version" state.

See [docs/design/DATA_MODEL.md](../design/DATA_MODEL.md) for the full trait definition.
