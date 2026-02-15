# Milestone 1: Core Data Model + CozoDB Store

## Goal

Implement `svt-core` with Rust type definitions, `GraphStore` trait, and CozoDB backend. Write + Read + basic traversal operations. Thorough tests including property-based.

## Approach

Vertical slices — each slice delivers types + trait methods + CozoDB implementation + tests. Working, tested code after every slice.

## Rust Types

All types live in `crates/core/src/model.rs` (or `model/` module as it grows).

### Enums

- `NodeKind` — System, Service, Component, Unit
- `EdgeKind` — Contains, Depends, Calls, Implements, Extends, DataFlow, Exports
- `Provenance` — Design, Analysis, Import, Inferred
- `SnapshotKind` — Design, Analysis, Import
- `Severity` — Error, Warning, Info
- `Direction` — Outgoing, Incoming, Both

`NodeKind` and `EdgeKind` are enums (closed set — architectural concepts). `sub_kind` and constraint `kind` are strings (open set — extensible without code changes).

### Type Aliases

- `Version = u64`
- `NodeId = String` (UUID)
- `EdgeId = String` (UUID)

### Structs

- `Node` — id, canonical_path, qualified_name?, kind, sub_kind, name, language?, provenance, source_ref?, metadata?
- `Edge` — id, source, target, kind, provenance, metadata?
- `Constraint` — id, kind (String), name, scope, target?, params?, message, severity
- `Snapshot` — version, kind, commit_ref?, created_at, metadata?
- `NodeFilter` — kind?, sub_kind?, language?

All derive `Debug, Clone, Serialize, Deserialize`. Enums also derive `PartialEq, Eq`.

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    NodeNotFound(NodeId),
    VersionNotFound(Version),
    DuplicateNode(String),
    DuplicateEdge(String),
    InvalidReference { edge_id, node_id },
    Internal(String),
}
```

## GraphStore Trait (Milestone 1 Scope)

### Snapshot Management

- `create_snapshot(kind, commit_ref?) -> Result<Version>`
- `list_snapshots() -> Result<Vec<Snapshot>>`
- `latest_version(kind) -> Result<Option<Version>>`

### Node Operations

- `add_node(version, node) -> Result<()>`
- `add_nodes_batch(version, nodes) -> Result<()>`
- `get_node(version, id) -> Result<Option<Node>>`
- `get_node_by_path(version, canonical_path) -> Result<Option<Node>>`

### Edge Operations

- `add_edge(version, edge) -> Result<()>`
- `add_edges_batch(version, edges) -> Result<()>`
- `get_edges(version, node_id, direction, kind?) -> Result<Vec<Edge>>`

### Containment Traversal

- `get_children(version, node_id) -> Result<Vec<Node>>`
- `get_parent(version, node_id) -> Result<Option<Node>>`
- `query_ancestors(version, node_id) -> Result<Vec<Node>>`
- `query_descendants(version, node_id, filter?) -> Result<Vec<Node>>`

### Dependency Traversal

- `query_dependencies(version, node_id, transitive) -> Result<Vec<Node>>`
- `query_dependents(version, node_id, transitive) -> Result<Vec<Node>>`

### Constraint Storage

- `add_constraint(version, constraint) -> Result<()>`
- `get_constraints(version) -> Result<Vec<Constraint>>`

### Version Management

- `compact(keep_versions) -> Result<()>`

### Deferred to Later Milestones

- `query_subgraph`, `query_paths`, `detect_cycles`
- `query_aggregated_edges`, `query_component_dependencies`
- `evaluate_constraints`, `evaluate_constraint`, `compare_versions`

## CozoDB Implementation

### CozoStore struct

- `new_in_memory()` — for tests
- `new_persistent(path)` — SQLite-backed for real use
- `init_schema()` — creates all four relations (snapshots, nodes, edges, constraints), idempotent

### Query patterns

- **Writes:** CozoDB `:put` operations with input relations
- **Reads:** Pattern matching against stored relations with bound parameters
- **Recursive traversal:** Datalog recursive rules for ancestors, descendants, transitive dependencies
- **Type conversion:** Private module handles Node/Edge ↔ CozoDB row mapping

### CozoDB Relations

```
:create snapshots { version: Int => kind, commit_ref?, created_at, metadata? }
:create nodes { id: String, version: Int => canonical_path, qualified_name?, kind, sub_kind, name, language?, provenance, source_ref?, metadata? }
:create edges { id: String, version: Int => source, target, kind, provenance, metadata? }
:create constraints { id: String, version: Int => kind, name, scope, target?, params?, message, severity }
```

## Testing Strategy

### Unit tests

In `crates/core/src/` alongside code, `#[cfg(test)]` modules. Test type conversions, canonical path utilities. All use `CozoStore::new_in_memory()`.

### Integration tests

In `crates/core/tests/`. Test GraphStore trait through CozoDB. Build realistic graphs, verify query results. Test names describe behaviour.

### Property-based tests (proptest)

- N nodes added then queried returns exactly N
- Ancestor chains have no duplicates
- Contains edges from valid canonical paths never form cycles
- Transitive dependencies are a superset of direct dependencies
- Compact preserves kept versions and removes the rest

### Test fixtures

Shared helper module with common graph builders:
- `create_simple_service()` — single service with components and units
- `create_layered_architecture()` — multi-layer service with ordered dependencies

## Slice Breakdown

### Slice 1: Foundation + Snapshot Management

**Types:** Version, SnapshotKind, Snapshot, StoreError
**Trait:** create_snapshot, list_snapshots, latest_version
**CozoDB:** CozoStore struct, constructors, init_schema, snapshot queries
**Tests:** Create returns incrementing versions, latest_version filters by kind, list returns all in order, property: N creates produce 1..N

### Slice 2: Node CRUD

**Types:** NodeId, NodeKind, Provenance, Node
**Trait:** add_node, add_nodes_batch, get_node, get_node_by_path
**CozoDB:** Node `:put`, query by id, query by canonical path, type conversion layer
**Tests:** Round-trip by id and path, duplicate error, batch add 100, optional fields as None, property: batch add N then get_by_path finds each

### Slice 3: Edge CRUD

**Types:** EdgeId, EdgeKind, Direction, Edge
**Trait:** add_edge, add_edges_batch, get_edges
**CozoDB:** Edge `:put`, query with direction and kind filter
**Tests:** Outgoing/incoming/both directions, kind filter, batch add, property: N edges from A then get_edges(A, Outgoing) returns N

### Slice 4: Containment Traversal

**Types:** NodeFilter
**Trait:** get_children, get_parent, query_ancestors, query_descendants
**CozoDB:** Join edges(contains) with nodes, recursive Datalog for ancestors/descendants, NodeFilter conditions
**Tests:** Children of parent, children of leaf (empty), parent of root (None), ancestors to root, descendants of subtree, descendants with filter, 5-level hierarchy, property: no duplicate ancestors, descendants of root = all non-root

### Slice 5: Dependency Traversal

**Trait:** query_dependencies, query_dependents
**CozoDB:** Direct dependency join, recursive Datalog for transitive
**Tests:** Direct deps, transitive chains (A→B→C), diamond handling, dependents reverse, empty deps, property: direct subset of transitive, no self in transitive

### Slice 6: Constraint Storage

**Types:** Constraint, Severity
**Trait:** add_constraint, get_constraints
**CozoDB:** Constraint `:put` and query by version
**Tests:** Round-trip, multiple constraints per version, empty version, optional fields, version scoping

### Slice 7: Validation

**Module:** crates/core/src/validation.rs
**Functions:** validate_contains_acyclic, validate_referential_integrity
**CozoDB:** Cycle detection via recursive Datalog, integrity check for dangling edge references
**Tests:** Clean graph passes, cycle detected, dangling reference flagged, property: canonical-path-derived graphs have no cycles
