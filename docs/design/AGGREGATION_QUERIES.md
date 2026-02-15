# Aggregation Queries

This document describes how unit-level edges are rolled up to component and service-level views for display in the UI. See [DATA_MODEL.md](./DATA_MODEL.md) for the underlying schema.

## Problem

The UI shows architecture at multiple abstraction levels, but stored edges are between units. A service-level view needs "service A depends on service B (weight: 47)" — not 47 individual function-to-class edges.

## Three Levels of Aggregation

### 1. Unit → Component Rollup

"How do modules relate to each other within a service?"

```datalog
# Aggregate depends edges between units into component-level edges
?[source_component, target_component, count(edge_id)] :=
    *edges{id: edge_id, source: src_id, target: tgt_id,
           kind: "depends", version: v},
    *edges{source: source_component, target: src_id,
           kind: "contains", version: v},
    *edges{source: target_component, target: tgt_id,
           kind: "contains", version: v},
    *nodes{id: source_component, kind: "component", version: v},
    *nodes{id: target_component, kind: "component", version: v},
    source_component != target_component
```

Result: "component A has N dependency edges pointing to component B."

### 2. Component → Service Rollup

"How do services relate to each other?"

Requires recursive ancestor lookup since components may be nested:

```datalog
# Find the service ancestor of a node
service_of[node_id, service_id] :=
    *edges{source: service_id, target: node_id, kind: "contains", version: v},
    *nodes{id: service_id, kind: "service", version: v}

service_of[node_id, service_id] :=
    *edges{source: parent_id, target: node_id, kind: "contains", version: v},
    service_of[parent_id, service_id]

# Aggregate to service level
?[source_service, target_service, edge_kind, count(edge_id)] :=
    *edges{id: edge_id, source: src_id, target: tgt_id,
           kind: edge_kind, version: v},
    service_of[src_id, source_service],
    service_of[tgt_id, target_service],
    source_service != target_service,
    edge_kind in ["depends", "calls", "data_flow"]
```

### 3. Arbitrary Ancestor Rollup

The UI lets the user navigate to any level. When viewing a specific node, edges are aggregated relative to that node's direct children:

```datalog
# Edges between direct children of a given parent node
?[source_child, target_child, edge_kind, count(edge_id)] :=
    *edges{source: parent_id, target: source_child, kind: "contains", version: v},
    *edges{source: parent_id, target: target_child, kind: "contains", version: v},
    descendant_of[src_id, source_child],
    descendant_of[tgt_id, target_child],
    *edges{id: edge_id, source: src_id, target: tgt_id,
           kind: edge_kind, version: v},
    source_child != target_child,
    edge_kind in ["depends", "calls", "data_flow"]
```

This generalises the pattern — any node in the hierarchy can be the aggregation root.

## Which Edge Kinds Are Aggregated?

| Edge kind | Aggregated? | Notes |
|---|---|---|
| `depends` | Yes | Weight = count of underlying edges |
| `calls` | Yes | Weight = count of underlying edges |
| `data_flow` | Yes | But typically already at high level |
| `contains` | No | Defines the hierarchy, not aggregated |
| `implements` | No | Structurally specific, not rolled up |
| `extends` | No | Structurally specific, not rolled up |
| `exports` | No | Defines visibility, not rolled up |

## Aggregated Edge Metadata

```json
{
  "weight": 47,
  "edge_kinds": ["depends", "calls"],
  "sample_edges": ["edge-123", "edge-456"]
}
```

`sample_edges` provides drill-down context — the UI can show "click to see the 47 underlying dependencies."

## Performance

Recursive queries over large graphs can be expensive. The `GraphStore` trait exposes `query_aggregated_edges` and `query_component_dependencies` methods so backends can optimise these with:

- Materialised views (pre-computed rollups updated on write)
- Query result caching
- Indexed containment paths for fast ancestor lookups

The choice of optimisation strategy is backend-specific — CozoDB may use stored rules, SurrealDB may use pre-computed tables.
