# Interchange Format

This document defines the import/export format for snapshots. See [DATA_MODEL.md](./DATA_MODEL.md) for the underlying schema.

## Overview

The interchange format serves multiple use cases:

1. **Design authoring** — humans write design models by hand
2. **Version control** — check snapshots into git, diff between commits
3. **Import from external tools** — ingest C4/Structurizr, OpenAPI, etc.
4. **CI artifacts** — conformance reports as build output
5. **Tool interop** — other tools consume the exported graph

Two serialisation formats share one schema:

- **YAML** — for human authoring and git-friendly interchange
- **JSON** — for machine consumption, CI output, and API responses

## Format Version

All interchange files include a format identifier for forward compatibility:

```yaml
format: svt/v1
```

Breaking changes to the schema increment the version. Importers should reject unrecognised versions with a clear error.

## Snapshot Export

### YAML (human-authored design)

```yaml
format: svt/v1
kind: design
version: 3
metadata:
  created_at: "2026-02-15T10:00:00Z"
  author: "pidster"

nodes:
  - canonical_path: /payments-service
    kind: service
    sub_kind: crate
    name: payments-service

  - canonical_path: /payments-service/handlers
    kind: component
    sub_kind: module
    name: handlers

  - canonical_path: /payments-service/handlers/create-order
    kind: unit
    sub_kind: function
    name: create-order

  - canonical_path: /payments-service/models
    kind: component
    sub_kind: module
    name: models

  - canonical_path: /payments-service/models/order
    kind: unit
    sub_kind: struct
    name: order

edges:
  - source: /payments-service/handlers/create-order
    target: /payments-service/models/order
    kind: depends

  - source: /payments-service
    target: /payments-service/handlers
    kind: contains

  - source: /payments-service
    target: /payments-service/models
    kind: contains

  - source: /payments-service/handlers
    target: /payments-service/handlers/create-order
    kind: contains

  - source: /payments-service/models
    target: /payments-service/models/order
    kind: contains

constraints:
  - name: no-internal-access
    kind: must_not_depend
    scope: /user-service/**
    target: /payments-service/internal/**
    message: "User service must not access payment internals"
    severity: error
```

### JSON (machine-generated analysis)

```json
{
  "format": "svt/v1",
  "kind": "analysis",
  "version": 5,
  "metadata": {
    "created_at": "2026-02-15T10:30:00Z",
    "commit_ref": "abc123f",
    "analyzer": "svt-analyzer/0.1.0",
    "language": "rust"
  },
  "nodes": [
    {
      "canonical_path": "/payments-service",
      "qualified_name": "payments_service",
      "kind": "service",
      "sub_kind": "crate",
      "name": "payments-service",
      "language": "rust",
      "metadata": {}
    }
  ],
  "edges": [
    {
      "source": "/payments-service/handlers/create-order",
      "target": "/payments-service/models/order",
      "kind": "depends",
      "metadata": {}
    }
  ],
  "constraints": []
}
```

### Design Choices

- **Edges reference canonical paths, not UUIDs** — human-readable and diffable. UUIDs are assigned on import into the graph store.
- **`format: svt/v1`** — versioned for forward compatibility.
- **Provenance is implied** by the top-level `kind` — a `design` file has design provenance, an `analysis` file has analysis provenance. No need to repeat it on every node.
- **`contains` edges are explicit** — the format is self-describing. See "Nested Shorthand" below for a more compact alternative.

## Nested Shorthand

For design authoring, explicit `contains` edges are tedious. An alternative nested syntax is supported:

```yaml
format: svt/v1
kind: design
version: 1

nodes:
  - canonical_path: /payments-service
    kind: service
    children:
      - canonical_path: /payments-service/handlers
        kind: component
        children:
          - canonical_path: /payments-service/handlers/create-order
            kind: unit
            sub_kind: function
      - canonical_path: /payments-service/models
        kind: component
        children:
          - canonical_path: /payments-service/models/order
            kind: unit
            sub_kind: struct
```

The importer flattens this into nodes + `contains` edges. Both flat and nested forms are valid input. Export always produces flat form for consistency.

## Inferred Fields

To reduce authoring burden, some fields can be omitted and inferred on import:

| Field | Inference rule |
|-------|---------------|
| `name` | Last segment of `canonical_path` |
| `sub_kind` | Defaults to the generic sub-kind for the `kind` (e.g., `component` defaults to `module`) |
| `contains` edges | Inferred from canonical path hierarchy if using nested shorthand |
| `provenance` | Inferred from top-level `kind` |

## Conformance Report Format

Conformance reports are CI artifacts. Always JSON for machine consumption.

```json
{
  "format": "svt/v1",
  "kind": "conformance_report",
  "design_version": 3,
  "analysis_version": 5,
  "metadata": {
    "created_at": "2026-02-15T11:00:00Z",
    "commit_ref": "abc123f"
  },
  "summary": {
    "constraints_passed": 12,
    "constraints_failed": 2,
    "constraints_warned": 1,
    "unimplemented": 3,
    "undocumented": 7
  },
  "constraint_results": [
    {
      "constraint": "no-internal-access",
      "status": "fail",
      "severity": "error",
      "message": "User service must not access payment internals",
      "violations": [
        {
          "source": "/user-service/auth/validate",
          "target": "/payments-service/internal/ledger",
          "edge_kind": "depends",
          "source_ref": "src/auth/validate.rs:42"
        }
      ]
    }
  ],
  "unimplemented": [
    {
      "canonical_path": "/payments-service/health-check",
      "kind": "unit",
      "message": "Defined in design but not found in analysis"
    }
  ],
  "undocumented": [
    {
      "canonical_path": "/payments-service/handlers/legacy-import",
      "kind": "unit",
      "message": "Found in analysis but not defined in design"
    }
  ]
}
```

## Markdown + Mermaid Export

For human-readable reports, a Markdown export with embedded Mermaid diagrams:

```markdown
# Conformance Report

Design v3 vs Analysis v5 | 2026-02-15

## Summary

| Status | Count |
|--------|-------|
| Constraints passed | 12 |
| Constraints failed | 2 |
| Constraints warned | 1 |
| Unimplemented (design only) | 3 |
| Undocumented (analysis only) | 7 |

## Constraint Violations

### no-internal-access (FAIL)

> User service must not access payment internals

| Source | Target | Location |
|--------|--------|----------|
| /user-service/auth/validate | /payments-service/internal/ledger | src/auth/validate.rs:42 |

## Architecture Overview

    ```mermaid
    graph LR
        user-service -->|depends| payments-service
        payments-service -->|depends| database-service
        style user-service fill:#f66
    ```

## Unimplemented

- `/payments-service/health-check` (unit)

## Undocumented

- `/payments-service/handlers/legacy-import` (unit)
```

The Mermaid diagram is generated from the service-level aggregated view, with conformance status driving node/edge styling (red for violations, grey for unimplemented, etc.).

## Import Behaviour

When importing a file into the graph store:

1. **Validate** the format version and schema
2. **Create a snapshot** with the next version number
3. **Assign UUIDs** to all nodes and edges (canonical paths are the external identity, UUIDs are internal)
4. **Resolve canonical paths** to existing nodes where applicable (for incremental updates)
5. **Infer missing fields** (name, sub_kind, contains edges, provenance)
6. **Insert** nodes, edges, and constraints into the store
7. **Return** the snapshot version for reference

## Export Behaviour

When exporting a snapshot from the graph store:

1. **Query** all nodes, edges, and constraints for the specified version
2. **Replace UUIDs** with canonical paths in edge source/target fields
3. **Serialise** to the requested format (YAML or JSON)
4. **Include** format version and snapshot metadata
