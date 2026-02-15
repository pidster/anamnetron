# Data Model

## Overview

The data model is a directed, typed property graph stored in an embedded graph database (behind a `GraphStore` trait). Nodes represent software elements at varying levels of abstraction. Edges represent relationships between them. Both carry provenance metadata indicating their origin and a version number linking them to a snapshot.

## Naming and Identity

### Canonical Path

Every node has a **canonical path** — a language-neutral, forward-slash-separated, lowercase kebab-case path derived from its position in the containment hierarchy:

```
/payments-service/handlers/create-order
/payments-service/models/order
/api-gateway/routes/health-check
```

The canonical path is the **primary matching key** for conformance. Design authors use canonical paths. Analyzers produce canonical paths. Conformance compares canonical paths.

### Naming Convention

Canonical path naming rules (defined in `svt-core`):

- Forward-slash separated segments
- Lowercase kebab-case for each segment (`create-order`, not `CreateOrder` or `create_order`)
- Root starts with `/`
- No trailing slash
- Segments correspond to `contains` hierarchy levels

### Language-Specific Mapping

Each analyzer implements a bidirectional mapping between language-specific qualified names and canonical paths:

| Language | Qualified Name | Canonical Path |
|----------|---------------|----------------|
| Rust | `payments_service::handlers::create_order` | `/payments-service/handlers/create-order` |
| Java | `com.example.payments.handlers.CreateOrder` | `/payments-service/handlers/create-order` |
| Python | `payments_service.handlers.create_order` | `/payments-service/handlers/create-order` |
| C# | `Payments.Handlers.CreateOrder` | `/payments-service/handlers/create-order` |
| TypeScript | `@payments/handlers/createOrder` | `/payments-service/handlers/create-order` |

Mapping rules are **convention-based in core** with **analyzer overrides** for language-specific edge cases. The core defines the canonical form; analyzers implement `to_canonical()` and `from_canonical()` conversions. Analyzers may override the default mapping when the convention doesn't fit (e.g., Java package prefixes like `com.example` that have no architectural meaning).

### Identity Fields

- `id` — internal unique ID (UUID), used for edge references
- `canonical_path` — language-neutral matching key, used for conformance and cross-language identity
- `qualified_name` — language-specific form, used for source navigation (null for design nodes)

## Nodes

Every node has a `kind` (abstraction level) and `sub_kind` (language-specific type).

### Node Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Internal unique identifier (UUID) |
| `version` | Int | Yes | Snapshot version this node belongs to |
| `canonical_path` | String | Yes | Language-neutral path (`/system/service/component/unit`) |
| `qualified_name` | String | No | Language-specific form, null for design nodes |
| `kind` | NodeKind | Yes | Abstraction level |
| `sub_kind` | String | Yes | Language-specific or domain-specific type |
| `name` | String | Yes | Simple name (last segment of canonical path) |
| `language` | String | No | Source language (`rust`, `typescript`, `python`, `csharp`, `java`, or null for design nodes) |
| `provenance` | Provenance | Yes | Origin of this knowledge |
| `source_ref` | String | No | File path, line number, or external URL |
| `metadata` | Json | No | Extensible key-value properties |

### Node Kinds and Sub-Kinds

| Kind | Description | Sub-Kinds by Language |
|------|-------------|----------------------|
| **system** | Top-level boundary, typically a repository or deployment | `workspace` (Rust), `monorepo` (JS/TS), `solution` (.NET), `repository` (generic), `project` (Maven) |
| **service** | Deployable or distributable unit | `crate` (Rust), `package` (JS/TS, Python), `assembly` (.NET), `module` (Java/Maven) |
| **component** | Logical grouping within a service | `module` (Rust, Python), `namespace` (C#, TS), `package` (Java), `directory` (generic) |
| **unit** | Individual code element | See table below |

### Unit Sub-Kinds

| Sub-Kind | Languages | Description |
|----------|-----------|-------------|
| `class` | Java, C#, Python, TS/JS | Class definition |
| `struct` | Rust, C# | Struct/value type |
| `enum` | Rust, Java, C#, TS, Python | Enumeration |
| `trait` | Rust | Trait definition |
| `interface` | Java, C#, TS | Interface definition |
| `protocol` | Python | Protocol (structural typing) |
| `function` | All | Standalone function |
| `method` | All | Method on a type |
| `type_alias` | Rust, TS | Type alias |
| `constant` | All | Constant or static value |

This is not exhaustive — the `sub_kind` field is a string, not an enum, to allow extension without schema changes. The above are the recognized built-in sub-kinds.

### Containment Hierarchy

The `contains` edge defines a tree structure. The hierarchy is **fully recursive** — any node can contain nodes of equal or lower abstraction level:

```
system
  └─ service
       └─ component
            └─ component          (nested modules, sub-packages)
                 └─ unit
                      └─ unit     (inner class, nested function)
                           └─ unit  (method on inner class)
```

Node `kind` describes what the node *is*, not its depth in the tree. A `component` may appear at depth 3 or depth 5.

**Invariant:** `contains` edges must not form cycles. This is enforced by validation and tested with property-based tests.

## Edges

### Edge Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique identifier |
| `version` | Int | Yes | Snapshot version this edge belongs to |
| `source` | String | Yes | Source node ID |
| `target` | String | Yes | Target node ID |
| `kind` | EdgeKind | Yes | Relationship type |
| `provenance` | Provenance | Yes | Origin of this knowledge |
| `metadata` | Json | No | Extensible key-value properties (weight, async, protocol, etc.) |

### Edge Kinds

| Kind | Description | Typical Volume | Example |
|------|-------------|----------------|---------|
| **contains** | Hierarchical nesting (parent → child) | Proportional to node count | `crate → module`, `module → struct` |
| **depends** | Import/use relationship | Moderate | `module A imports module B` |
| **calls** | Runtime invocation | High (cross-boundary stored, intra-component on demand) | `function X calls function Y` |
| **implements** | Fulfills a contract | Low | `struct implements trait`, `class implements interface` |
| **extends** | Inheritance relationship | Low | `class extends base class` |
| **data_flow** | Data movement between elements | Low (primarily design-mode) | `data flows from queue to processor` |
| **exports** | Public visibility boundary | Moderate | `module exports struct` |

### Edge Volume Considerations

Not all edge kinds are equal in volume:

- **Always stored:** `contains`, `implements`, `extends`, `exports`, `data_flow` — these are structurally sparse and architecturally significant.
- **Stored, aggregated for display:** `depends` — stored at the unit level, aggregated via queries for component/service-level views.
- **Stored selectively:** `calls` — cross-boundary calls are stored (architecturally interesting). Intra-component call graphs may be computed on demand.

## Versioning

### Snapshots

The model is versioned through **snapshots**. Each analysis run, design revision, or import creates a new snapshot with a monotonically increasing version number.

| Field | Type | Description |
|-------|------|-------------|
| `version` | Int | Monotonically increasing version number (primary key) |
| `kind` | String | `design`, `analysis`, or `import` |
| `commit_ref` | String? | Git commit hash, if applicable |
| `created_at` | String | Timestamp (informational, not used for ordering) |
| `metadata` | Json? | Additional context (analyzer version, source details, etc.) |

### How Versioning Works

- Nodes and edges carry a `version` field linking them to a snapshot.
- The **current state** is the latest snapshot of each kind.
- **Drift detection** compares two analysis snapshots:

```datalog
# Nodes added in version 5 that weren't in version 3
?[id, name, kind] :=
    *nodes{id, name, kind, version: 5},
    not *nodes{id, version: 3}

# Nodes removed (in version 3 but not version 5)
?[id, name, kind] :=
    *nodes{id, name, kind, version: 3},
    not *nodes{id, version: 5}
```

- **Conformance** compares the latest design snapshot against the latest analysis snapshot.
- **Snapshot compaction** — old snapshots can be pruned to control storage growth. Policy is configurable (keep last N, keep tagged snapshots, etc.).

## Provenance

| Value | Description |
|-------|-------------|
| `design` | Human-authored, prescriptive (design mode) |
| `analysis` | Machine-derived from code (discovery mode) |
| `import` | Ingested from an external knowledge source |
| `inferred` | Derived from heuristics or patterns |

## Conformance

Conformance status is **computed, not stored** — derived by comparing design-provenance and analysis-provenance subgraphs via their canonical paths.

| Status | Meaning |
|--------|---------|
| `matched` | Exists in both design and analysis, consistent |
| `violation` | Exists in analysis but contradicts a design constraint |
| `unimplemented` | Exists in design but not found in analysis |
| `undocumented` | Exists in analysis but absent from design |

Conformance is expressed as Datalog queries against the graph store, making it naturally extensible to user-defined rules in the future.

## Metadata Conventions

Well-known keys that analyzers should populate when available. These are **conventions, not enforced schema** — missing keys are acceptable.

### Node Metadata

| Key | Type | Applies to | Description |
|-----|------|-----------|-------------|
| `visibility` | `public \| private \| protected \| internal` | units, components | Access modifier |
| `is_async` | bool | functions, methods | Async function/method |
| `is_abstract` | bool | classes, methods | Abstract class/method |
| `is_static` | bool | methods, fields | Static member |
| `is_deprecated` | bool | any | Marked deprecated |
| `doc_comment` | string | any | Documentation text |
| `file_path` | string | any | Source file path |
| `line_start` | int | units | Start line in source |
| `line_end` | int | units | End line in source |
| `annotations` | string[] | units | Decorators/attributes/annotations |
| `generic_params` | string[] | units | Type parameters |

### Edge Metadata

| Key | Type | Applies to | Description |
|-----|------|-----------|-------------|
| `weight` | int | aggregated edges | Count of underlying edges |
| `protocol` | string | data_flow | HTTP, gRPC, AMQP, etc. |
| `is_async` | bool | calls | Async invocation |
| `is_conditional` | bool | calls, depends | Behind a feature flag or conditional import |

## CozoDB Relations

```
:create snapshots {
    version: Int         =>
    kind: String,
    commit_ref: String?,
    created_at: String,
    metadata: Json?,
}

:create nodes {
    id: String, version: Int  =>
    canonical_path: String,
    qualified_name: String?,
    kind: String,
    sub_kind: String,
    name: String,
    language: String?,
    provenance: String,
    source_ref: String?,
    metadata: Json?,
}

:create edges {
    id: String, version: Int  =>
    source: String,
    target: String,
    kind: String,
    provenance: String,
    metadata: Json?,
}
```

Note: These are CozoDB-specific. The `GraphStore` trait in `svt-core` defines the abstract interface; alternative backends (e.g., SurrealDB) implement the same operations over their own schema.
