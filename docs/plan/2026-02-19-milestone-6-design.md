# Milestone 6: CLI Export + Additional Constraints — Design

## Overview

Complete the core CLI toolset and make conformance checking comprehensive. Two independent workstreams:

1. **Three new constraint evaluators** — `boundary`, `must_contain`, `max_fan_in`
2. **CLI export command** — `svt export --format mermaid|json`

After this milestone, all 10 constraints in `design/architecture.yaml` will be fully evaluated (no more `NotEvaluable`), and the CLI can produce diagram and data exports.

## Constraint Evaluators

All three go in `crates/core/src/conformance.rs`, following the same pattern as `evaluate_constraint_must_not_depend`.

### `boundary` (scope_only)

**Semantics:** No node *outside* the scope pattern may have a `Depends` edge targeting a node *inside* the scope. Enforces encapsulation — internal implementation details stay hidden. One-directional: only checks inward access, not outward.

**Algorithm:**
1. Collect all nodes matching the scope pattern → `scoped_ids`
2. Collect all `Depends` edges in the version
3. For each edge where `target ∈ scoped_ids` and `source ∉ scoped_ids` → violation

**Violation reporting:** `source_path` is the external node, `target_path` is the leaked internal node.

**Params:** `{ access: scope_only }` — currently the only access mode. The params field is validated but the behaviour is fixed for this milestone.

### `must_contain`

**Semantics:** The scope node must have at least one child whose name matches `params.child_pattern` and (optionally) whose kind matches `params.child_kind`.

**Algorithm:**
1. Find the node matching `scope` (exact match, not glob)
2. Get its children via `store.get_children()`
3. Check that at least one child has `name` matching `child_pattern` and `kind` matching `child_kind` (if specified)

**Evaluability:** Works against both design and analysis snapshots. No special handling needed.

### `max_fan_in`

**Semantics:** Count incoming edges of kind `params.edge_kind` to the scope node. Fail if count exceeds `params.limit`.

**Algorithm:**
1. Find the node matching `scope`
2. Get incoming edges of kind `edge_kind`
3. If `params.level` is specified, only count edges whose source node is at that `NodeKind` level
4. Compare count against `limit`

**Violation reporting:** Single violation with the count in the message. Threshold check, not per-edge.

### Integration into `evaluate_constraints()`

The existing `evaluate_constraints()` function has a `match constraint.kind.as_str()` block that currently handles `"must_not_depend"` and falls through to `NotEvaluable` for anything else. Add three new arms:

```
"boundary" => evaluate_constraint_boundary(store, constraint, eval_version)?
"must_contain" => evaluate_constraint_must_contain(store, constraint, eval_version)?
"max_fan_in" => evaluate_constraint_max_fan_in(store, constraint, eval_version)?
```

## CLI Export Command

### Command Interface

```
svt export --format mermaid|json [--version V] [--output FILE]
```

- `--format` (required): `mermaid` or `json`
- `--version`: snapshot version to export (defaults to latest design)
- `--output` / `-o`: write to file instead of stdout

### Mermaid Output

Generates a Mermaid `flowchart TD` diagram:
- Containment hierarchy expressed via `subgraph` blocks
- Non-containment edges rendered as arrows with kind labels
- Node IDs sanitised for Mermaid compatibility (replace `/` and `-`)

Example:
```
flowchart TD
    subgraph svt["/svt"]
        subgraph core["/svt/core"]
            core_model["/svt/core/model"]
            core_store["/svt/core/store"]
        end
        subgraph cli["/svt/cli"]
        end
    end
    cli -->|depends| core
    core_store -->|depends| core_model
```

**Implementation:** New module `crates/core/src/export/mermaid.rs`. Pure function: `fn to_mermaid(store: &impl GraphStore, version: Version) -> Result<String>`.

### JSON Export

Re-uses existing `interchange_store::export_json`. The CLI command wires it to stdout/file.

### Module Structure

```
crates/core/src/
  export/
    mod.rs          — pub mod mermaid;
    mermaid.rs      — Mermaid flowchart generation
```

The export module lives in `svt-core` so it's available to both CLI and server.

## Testing Strategy

### Constraint Tests

- **`boundary`**: Pass (no external deps on internal nodes), fail (external depends on internal), edge case (internal-to-internal deps allowed)
- **`must_contain`**: Pass (child exists matching pattern+kind), fail (no matching child), partial (name matches but kind doesn't)
- **`max_fan_in`**: Under limit, at limit, over limit. Level filtering.

### Export Tests

- **Mermaid**: Snapshot test (insta) for a small fixture graph. Verify subgraph nesting, edge labels.
- **JSON**: Round-trip test — export then re-import, verify node/edge counts match.
- **CLI integration**: `svt export --format mermaid` and `svt export --format json` produce non-empty output.

### Dog-food Validation

Final acceptance: run `svt check` against `design/architecture.yaml` and verify **zero** `NotEvaluable` results. All 10 constraints evaluate to Pass or Fail.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Boundary direction | Inward only | Matches encapsulation semantics; outward restriction is a different concern |
| Export formats | Mermaid + JSON | Text-based, git-friendly; covers both human and programmatic use |
| Export module location | `svt-core` | Available to CLI, server, and future WASM |
| Mermaid style | `flowchart TD` | Top-down matches containment hierarchy; readable |
| JSON export | Reuse interchange format | No new format to maintain |
