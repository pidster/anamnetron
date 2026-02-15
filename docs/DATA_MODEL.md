# Data Model

## Overview

The data model is a directed, typed property graph stored in an embedded graph database (behind a `GraphStore` trait). Nodes represent software elements at varying levels of abstraction. Edges represent relationships between them. Both carry provenance metadata indicating their origin.

## Nodes

Every node has a `kind` (abstraction level) and `sub_kind` (language-specific type).

### Node Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique identifier |
| `kind` | NodeKind | Yes | Abstraction level |
| `sub_kind` | String | Yes | Language-specific or domain-specific type |
| `name` | String | Yes | Human-readable name |
| `qualified_name` | String | Yes | Fully qualified name (e.g., `crate::module::Struct`) |
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

## Edges

### Edge Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique identifier |
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

## Provenance

| Value | Description |
|-------|-------------|
| `design` | Human-authored, prescriptive (design mode) |
| `analysis` | Machine-derived from code (discovery mode) |
| `import` | Ingested from an external knowledge source |
| `inferred` | Derived from heuristics or patterns |

## Conformance

Conformance status is **computed, not stored** — derived by comparing design-provenance and analysis-provenance subgraphs.

| Status | Meaning |
|--------|---------|
| `matched` | Exists in both design and analysis, consistent |
| `violation` | Exists in analysis but contradicts a design constraint |
| `unimplemented` | Exists in design but not found in analysis |
| `undocumented` | Exists in analysis but absent from design |

Conformance is expressed as Datalog queries against the graph store, making it naturally extensible to user-defined rules in the future.

## CozoDB Relations

```
:create nodes {
    id: String           =>
    kind: String,
    sub_kind: String,
    name: String,
    qualified_name: String,
    language: String?,
    provenance: String,
    source_ref: String?,
    metadata: Json?,
}

:create edges {
    id: String           =>
    source: String,
    target: String,
    kind: String,
    provenance: String,
    metadata: Json?,
}
```

Note: These are CozoDB-specific. The `GraphStore` trait in `svt-core` defines the abstract interface; alternative backends (e.g., SurrealDB) implement the same operations over their own schema.
