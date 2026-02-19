# Milestone 6: CLI Export + Additional Constraints — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement three new constraint evaluators (boundary, must_contain, max_fan_in) and a CLI export command (Mermaid + JSON), making all dog-food constraints fully evaluable.

**Architecture:** New constraint evaluator functions in `conformance.rs` following the existing `evaluate_constraint_must_not_depend` pattern. New `export/` module in svt-core for Mermaid generation. CLI gets an `Export` subcommand. Dog-food tests updated to assert zero `NotEvaluable` results.

**Tech Stack:** Rust, CozoDB (GraphStore trait), insta (snapshot tests), assert_cmd (CLI tests), clap (CLI args)

---

### Task 1: `boundary` Constraint — Tests

**Files:**
- Modify: `crates/core/src/conformance.rs` (tests module, starting at line 398)

**Step 1: Write failing tests for boundary constraint**

Add these tests to the `#[cfg(test)] mod tests` block in `conformance.rs`:

```rust
#[test]
fn boundary_passes_when_no_external_deps_on_internal() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let app = make_node("n1", "/app", NodeKind::System, "workspace");
    let store_mod = make_node("n2", "/app/store", NodeKind::Component, "module");
    let store_cozo = make_node("n3", "/app/store/cozo", NodeKind::Component, "module");
    let api = make_node("n4", "/app/api", NodeKind::Component, "module");
    store.add_node(v, &app).unwrap();
    store.add_node(v, &store_mod).unwrap();
    store.add_node(v, &store_cozo).unwrap();
    store.add_node(v, &api).unwrap();

    // api depends on store (allowed — store is not inside boundary)
    store.add_edge(v, &make_edge("e1", "n4", "n2", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "boundary".to_string(),
        name: "store-encapsulation".to_string(),
        scope: "/app/store/cozo/**".to_string(),
        target: None,
        params: Some(serde_json::json!({"access": "scope_only"})),
        message: "CozoDB internals must not leak".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Pass);
    assert!(result.violations.is_empty());
}

#[test]
fn boundary_fails_when_external_depends_on_internal() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let app = make_node("n1", "/app", NodeKind::System, "workspace");
    let store_mod = make_node("n2", "/app/store", NodeKind::Component, "module");
    let store_cozo = make_node("n3", "/app/store/cozo", NodeKind::Component, "module");
    let api = make_node("n4", "/app/api", NodeKind::Component, "module");
    store.add_node(v, &app).unwrap();
    store.add_node(v, &store_mod).unwrap();
    store.add_node(v, &store_cozo).unwrap();
    store.add_node(v, &api).unwrap();

    // api depends on store/cozo (FORBIDDEN — inside boundary)
    store.add_edge(v, &make_edge("e1", "n4", "n3", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "boundary".to_string(),
        name: "store-encapsulation".to_string(),
        scope: "/app/store/cozo/**".to_string(),
        target: None,
        params: Some(serde_json::json!({"access": "scope_only"})),
        message: "CozoDB internals must not leak".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Fail);
    assert_eq!(result.violations.len(), 1);
    assert_eq!(result.violations[0].source_path, "/app/api");
    assert_eq!(result.violations[0].target_path, Some("/app/store/cozo".to_string()));
}

#[test]
fn boundary_allows_internal_to_internal_deps() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let app = make_node("n1", "/app", NodeKind::System, "workspace");
    let cozo = make_node("n2", "/app/store/cozo", NodeKind::Component, "module");
    let cozo_inner = make_node("n3", "/app/store/cozo/queries", NodeKind::Unit, "module");
    store.add_node(v, &app).unwrap();
    store.add_node(v, &cozo).unwrap();
    store.add_node(v, &cozo_inner).unwrap();

    // internal depends on internal (allowed)
    store.add_edge(v, &make_edge("e1", "n3", "n2", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "boundary".to_string(),
        name: "store-encapsulation".to_string(),
        scope: "/app/store/cozo/**".to_string(),
        target: None,
        params: Some(serde_json::json!({"access": "scope_only"})),
        message: "CozoDB internals must not leak".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Pass);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core boundary -- --nocapture`
Expected: FAIL — `evaluate_constraint_boundary` does not exist yet.

---

### Task 2: `boundary` Constraint — Implementation

**Files:**
- Modify: `crates/core/src/conformance.rs` (add function before `structural_checks`, around line 178; update `evaluate_constraints` match at line 251)

**Step 1: Implement `evaluate_constraint_boundary`**

Add this function before `structural_checks()` (around line 178):

```rust
/// Evaluate a `boundary` constraint (scope_only access mode).
///
/// Checks that no node outside the scope pattern has a `Depends` edge
/// targeting a node inside the scope. Internal-to-internal deps are allowed.
pub fn evaluate_constraint_boundary(
    store: &impl GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let all_nodes = store.get_all_nodes(version)?;
    let depends_edges = store.get_all_edges(version, Some(EdgeKind::Depends))?;

    let mut scoped_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut id_to_path: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();

    for node in &all_nodes {
        id_to_path.insert(&node.id, &node.canonical_path);
        if canonical_path_matches(&node.canonical_path, &constraint.scope) {
            scoped_ids.insert(&node.id);
        }
    }

    let mut violations = Vec::new();
    for edge in &depends_edges {
        let target_in_scope = scoped_ids.contains(edge.target.as_str());
        let source_in_scope = scoped_ids.contains(edge.source.as_str());

        // Violation: external source depends on internal target
        if target_in_scope && !source_in_scope {
            violations.push(Violation {
                source_path: id_to_path
                    .get(edge.source.as_str())
                    .unwrap_or(&"")
                    .to_string(),
                target_path: Some(
                    id_to_path
                        .get(edge.target.as_str())
                        .unwrap_or(&"")
                        .to_string(),
                ),
                edge_id: Some(edge.id.clone()),
                edge_kind: Some(edge.kind),
                source_ref: None,
            });
        }
    }

    let status = if violations.is_empty() {
        ConstraintStatus::Pass
    } else {
        ConstraintStatus::Fail
    };

    Ok(ConstraintResult {
        constraint_name: constraint.name.clone(),
        constraint_kind: constraint.kind.clone(),
        status,
        severity: constraint.severity,
        message: constraint.message.clone(),
        violations,
    })
}
```

**Step 2: Wire into `evaluate_constraints`**

In `evaluate_constraints()` (line 251), add a new match arm:

```rust
"must_not_depend" => {
    evaluate_constraint_must_not_depend(store, constraint, eval_version)?
}
"boundary" => {
    evaluate_constraint_boundary(store, constraint, eval_version)?
}
```

**Step 3: Run tests to verify they pass**

Run: `cargo test -p svt-core boundary -- --nocapture`
Expected: All 3 boundary tests PASS.

**Step 4: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(core): implement boundary constraint evaluator"
```

---

### Task 3: `must_contain` Constraint — Tests

**Files:**
- Modify: `crates/core/src/conformance.rs` (tests module)

**Step 1: Write failing tests for must_contain constraint**

```rust
#[test]
fn must_contain_passes_when_child_exists() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let cli = make_node("n1", "/app/cli", NodeKind::Service, "crate");
    let cmds = make_node("n2", "/app/cli/commands", NodeKind::Component, "module");
    let check = make_node("n3", "/app/cli/commands/check", NodeKind::Unit, "function");
    store.add_node(v, &cli).unwrap();
    store.add_node(v, &cmds).unwrap();
    store.add_node(v, &check).unwrap();
    // Containment edges
    store.add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains)).unwrap();
    store.add_edge(v, &make_edge("e2", "n2", "n3", EdgeKind::Contains)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "must_contain".to_string(),
        name: "cli-has-check".to_string(),
        scope: "/app/cli/commands".to_string(),
        target: None,
        params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
        message: "CLI must have check command".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Pass);
}

#[test]
fn must_contain_fails_when_child_missing() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let cli = make_node("n1", "/app/cli", NodeKind::Service, "crate");
    let cmds = make_node("n2", "/app/cli/commands", NodeKind::Component, "module");
    store.add_node(v, &cli).unwrap();
    store.add_node(v, &cmds).unwrap();
    store.add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains)).unwrap();
    // No children of /app/cli/commands

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "must_contain".to_string(),
        name: "cli-has-check".to_string(),
        scope: "/app/cli/commands".to_string(),
        target: None,
        params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
        message: "CLI must have check command".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Fail);
    assert_eq!(result.violations.len(), 1);
}

#[test]
fn must_contain_fails_when_name_matches_but_kind_does_not() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let cmds = make_node("n1", "/app/cli/commands", NodeKind::Component, "module");
    // Child named "check" but kind is Component, not Unit
    let check = make_node("n2", "/app/cli/commands/check", NodeKind::Component, "module");
    store.add_node(v, &cmds).unwrap();
    store.add_node(v, &check).unwrap();
    store.add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "must_contain".to_string(),
        name: "cli-has-check".to_string(),
        scope: "/app/cli/commands".to_string(),
        target: None,
        params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
        message: "CLI must have check command".to_string(),
        severity: Severity::Error,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Fail);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core must_contain -- --nocapture`
Expected: FAIL — `evaluate_constraint_must_contain` does not exist yet.

---

### Task 4: `must_contain` Constraint — Implementation

**Files:**
- Modify: `crates/core/src/conformance.rs` (add function; update match)

**Step 1: Implement `evaluate_constraint_must_contain`**

Add after `evaluate_constraint_boundary`:

```rust
/// Evaluate a `must_contain` constraint.
///
/// Checks that the scope node has at least one child matching
/// `params.child_pattern` (by name) and optionally `params.child_kind` (by NodeKind).
pub fn evaluate_constraint_must_contain(
    store: &impl GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let scope_node = store.get_node_by_path(version, &constraint.scope)?;

    let scope_node = match scope_node {
        Some(n) => n,
        None => {
            return Ok(ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("Scope node '{}' not found", constraint.scope),
                violations: vec![],
            });
        }
    };

    let children = store.get_children(version, &scope_node.id)?;

    let child_pattern = constraint
        .params
        .as_ref()
        .and_then(|p| p.get("child_pattern"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let child_kind = constraint
        .params
        .as_ref()
        .and_then(|p| p.get("child_kind"))
        .and_then(|v| v.as_str());

    let has_match = children.iter().any(|child| {
        let name_matches = child.name == child_pattern;
        let kind_matches = child_kind
            .map(|k| {
                let kind_str = serde_json::to_string(&child.kind).unwrap_or_default();
                // Compare without quotes: serde produces "\"unit\""
                kind_str.trim_matches('"') == k
            })
            .unwrap_or(true);
        name_matches && kind_matches
    });

    if has_match {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Pass,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![],
        })
    } else {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Fail,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![Violation {
                source_path: constraint.scope.clone(),
                target_path: None,
                edge_id: None,
                edge_kind: None,
                source_ref: None,
            }],
        })
    }
}
```

**Step 2: Wire into `evaluate_constraints`**

Add to the match block:

```rust
"must_contain" => {
    evaluate_constraint_must_contain(store, constraint, eval_version)?
}
```

**Step 3: Run tests to verify they pass**

Run: `cargo test -p svt-core must_contain -- --nocapture`
Expected: All 3 must_contain tests PASS.

**Step 4: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(core): implement must_contain constraint evaluator"
```

---

### Task 5: `max_fan_in` Constraint — Tests

**Files:**
- Modify: `crates/core/src/conformance.rs` (tests module)

**Step 1: Write failing tests for max_fan_in constraint**

```rust
#[test]
fn max_fan_in_passes_when_under_limit() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let model = make_node("n1", "/app/model", NodeKind::Component, "module");
    let api = make_node("n2", "/app/api", NodeKind::Component, "module");
    store.add_node(v, &model).unwrap();
    store.add_node(v, &api).unwrap();

    // 1 incoming depends edge (limit is 5)
    store.add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "max_fan_in".to_string(),
        name: "model-fan-in".to_string(),
        scope: "/app/model".to_string(),
        target: None,
        params: Some(serde_json::json!({"edge_kind": "depends", "limit": 5})),
        message: "Model fan-in should be reasonable".to_string(),
        severity: Severity::Warning,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Pass);
}

#[test]
fn max_fan_in_fails_when_over_limit() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let model = make_node("n1", "/app/model", NodeKind::Component, "module");
    let a = make_node("n2", "/app/a", NodeKind::Component, "module");
    let b = make_node("n3", "/app/b", NodeKind::Component, "module");
    let c = make_node("n4", "/app/c", NodeKind::Component, "module");
    store.add_node(v, &model).unwrap();
    store.add_node(v, &a).unwrap();
    store.add_node(v, &b).unwrap();
    store.add_node(v, &c).unwrap();

    // 3 incoming depends edges (limit is 2)
    store.add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends)).unwrap();
    store.add_edge(v, &make_edge("e2", "n3", "n1", EdgeKind::Depends)).unwrap();
    store.add_edge(v, &make_edge("e3", "n4", "n1", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "max_fan_in".to_string(),
        name: "model-fan-in".to_string(),
        scope: "/app/model".to_string(),
        target: None,
        params: Some(serde_json::json!({"edge_kind": "depends", "limit": 2})),
        message: "Model fan-in too high".to_string(),
        severity: Severity::Warning,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
    assert_eq!(result.status, ConstraintStatus::Fail);
    assert_eq!(result.violations.len(), 1);
}

#[test]
fn max_fan_in_with_level_filter_counts_only_matching_kind() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let model = make_node("n1", "/app/model", NodeKind::Component, "module");
    let api = make_node("n2", "/app/api", NodeKind::Component, "module");
    let handler = make_node("n3", "/app/api/handler", NodeKind::Unit, "function");
    store.add_node(v, &model).unwrap();
    store.add_node(v, &api).unwrap();
    store.add_node(v, &handler).unwrap();

    // 2 incoming edges, but only 1 from component level
    store.add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends)).unwrap();
    store.add_edge(v, &make_edge("e2", "n3", "n1", EdgeKind::Depends)).unwrap();

    let constraint = Constraint {
        id: "c1".to_string(),
        kind: "max_fan_in".to_string(),
        name: "model-fan-in".to_string(),
        scope: "/app/model".to_string(),
        target: None,
        params: Some(serde_json::json!({"edge_kind": "depends", "limit": 1, "level": "component"})),
        message: "Model fan-in at component level".to_string(),
        severity: Severity::Warning,
    };
    store.add_constraint(v, &constraint).unwrap();

    let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
    // Only 1 component-level edge, limit is 1, so it passes
    assert_eq!(result.status, ConstraintStatus::Pass);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core max_fan_in -- --nocapture`
Expected: FAIL — `evaluate_constraint_max_fan_in` does not exist yet.

---

### Task 6: `max_fan_in` Constraint — Implementation

**Files:**
- Modify: `crates/core/src/conformance.rs` (add function; update match)

**Step 1: Implement `evaluate_constraint_max_fan_in`**

Add after `evaluate_constraint_must_contain`:

```rust
/// Evaluate a `max_fan_in` constraint.
///
/// Counts incoming edges of the specified kind to the scope node.
/// If `params.level` is specified, only counts edges from nodes of that `NodeKind`.
/// Fails if the count exceeds `params.limit`.
pub fn evaluate_constraint_max_fan_in(
    store: &impl GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let scope_node = store.get_node_by_path(version, &constraint.scope)?;

    let scope_node = match scope_node {
        Some(n) => n,
        None => {
            return Ok(ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("Scope node '{}' not found", constraint.scope),
                violations: vec![],
            });
        }
    };

    let params = constraint.params.as_ref();

    let edge_kind_str = params
        .and_then(|p| p.get("edge_kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("depends");

    let edge_kind_filter: Option<EdgeKind> =
        serde_json::from_str(&format!("\"{}\"", edge_kind_str)).ok();

    let limit = params
        .and_then(|p| p.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX) as usize;

    let level_filter: Option<NodeKind> = params
        .and_then(|p| p.get("level"))
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());

    // Get incoming edges of the specified kind
    let incoming = store.get_edges(
        version,
        &scope_node.id,
        Direction::Incoming,
        edge_kind_filter,
    )?;

    // Apply level filter if specified
    let fan_in = if let Some(level) = level_filter {
        incoming
            .iter()
            .filter(|edge| {
                store
                    .get_node(version, &edge.source)
                    .ok()
                    .flatten()
                    .map(|n| n.kind == level)
                    .unwrap_or(false)
            })
            .count()
    } else {
        incoming.len()
    };

    if fan_in <= limit {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Pass,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![],
        })
    } else {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Fail,
            severity: constraint.severity,
            message: format!(
                "{} (fan-in: {}, limit: {})",
                constraint.message, fan_in, limit
            ),
            violations: vec![Violation {
                source_path: constraint.scope.clone(),
                target_path: None,
                edge_id: None,
                edge_kind: edge_kind_filter,
                source_ref: None,
            }],
        })
    }
}
```

**Step 2: Wire into `evaluate_constraints`**

Add to the match block:

```rust
"max_fan_in" => {
    evaluate_constraint_max_fan_in(store, constraint, eval_version)?
}
```

**Step 3: Run tests to verify they pass**

Run: `cargo test -p svt-core max_fan_in -- --nocapture`
Expected: All 3 max_fan_in tests PASS.

**Step 4: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(core): implement max_fan_in constraint evaluator"
```

---

### Task 7: Mermaid Export Module — Tests

**Files:**
- Create: `crates/core/src/export/mod.rs`
- Create: `crates/core/src/export/mermaid.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod export`)

**Step 1: Create export module skeleton and write failing test**

Create `crates/core/src/export/mod.rs`:

```rust
//! Export graph data in various formats.

pub mod mermaid;
```

Create `crates/core/src/export/mermaid.rs` with just tests:

```rust
//! Mermaid flowchart export.

#[cfg(test)]
mod tests {
    use crate::interchange::parse_yaml;
    use crate::interchange_store::load_into_store;
    use crate::store::CozoStore;

    #[test]
    fn simple_graph_produces_valid_mermaid() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let output = super::to_mermaid(&store, version).unwrap();

        assert!(output.starts_with("flowchart TD"));
        assert!(output.contains("subgraph"));
        assert!(output.contains("depends"));
    }

    #[test]
    fn mermaid_contains_all_non_containment_edges() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/a
        kind: service
      - canonical_path: /app/b
        kind: service
edges:
  - source: /app/a
    target: /app/b
    kind: depends
  - source: /app/a
    target: /app/b
    kind: data_flow
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let output = super::to_mermaid(&store, version).unwrap();

        assert!(output.contains("depends"), "should contain depends edge");
        assert!(output.contains("data_flow"), "should contain data_flow edge");
    }

    #[test]
    fn mermaid_snapshot_test() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
        children:
          - canonical_path: /app/core/model
            kind: component
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let output = super::to_mermaid(&store, version).unwrap();
        insta::assert_snapshot!(output);
    }
}
```

Add to `crates/core/src/lib.rs` after the conformance module:

```rust
/// Export graph data in various formats (Mermaid, JSON).
#[cfg(feature = "store")]
pub mod export;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core mermaid -- --nocapture`
Expected: FAIL — `to_mermaid` does not exist yet.

---

### Task 8: Mermaid Export — Implementation

**Files:**
- Modify: `crates/core/src/export/mermaid.rs` (add implementation above tests)

**Step 1: Implement `to_mermaid`**

Add at the top of `mermaid.rs`, above the `#[cfg(test)]` block:

```rust
use crate::model::*;
use crate::store::{GraphStore, Result};

/// Sanitise a canonical path into a valid Mermaid node ID.
fn mermaid_id(path: &str) -> String {
    path.trim_start_matches('/')
        .replace('/', "_")
        .replace('-', "_")
}

/// Generate a Mermaid flowchart from a graph store version.
///
/// Containment hierarchy is expressed via `subgraph` blocks.
/// Non-containment edges are rendered as labelled arrows.
#[must_use]
pub fn to_mermaid(store: &impl GraphStore, version: Version) -> Result<String> {
    let nodes = store.get_all_nodes(version)?;
    let edges = store.get_all_edges(version, None)?;

    // Build parent map from Contains edges
    let id_to_path: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.canonical_path.as_str()))
        .collect();

    let mut parent_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for edge in &edges {
        if edge.kind == EdgeKind::Contains {
            if let (Some(&_src_path), Some(&target_path)) =
                (id_to_path.get(edge.source.as_str()), id_to_path.get(edge.target.as_str()))
            {
                parent_map.insert(target_path, id_to_path[edge.source.as_str()]);
            }
        }
    }

    // Find root nodes (no parent)
    let node_paths: Vec<&str> = nodes.iter().map(|n| n.canonical_path.as_str()).collect();
    let roots: Vec<&str> = node_paths
        .iter()
        .filter(|p| !parent_map.contains_key(*p))
        .copied()
        .collect();

    // Build children map
    let mut children_map: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for (&child, &parent) in &parent_map {
        children_map.entry(parent).or_default().push(child);
    }
    // Sort children for deterministic output
    for children in children_map.values_mut() {
        children.sort();
    }

    // Collect non-containment edges
    let dep_edges: Vec<_> = edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Contains)
        .collect();

    let mut out = String::new();
    out.push_str("flowchart TD\n");

    fn write_node(
        out: &mut String,
        path: &str,
        children_map: &std::collections::HashMap<&str, Vec<&str>>,
        indent: usize,
    ) {
        let pad = "    ".repeat(indent);
        let id = mermaid_id(path);

        if let Some(children) = children_map.get(path) {
            out.push_str(&format!("{pad}subgraph {id}[\"{path}\"]\n"));
            for child in children {
                write_node(out, child, children_map, indent + 1);
            }
            out.push_str(&format!("{pad}end\n"));
        } else {
            out.push_str(&format!("{pad}{id}[\"{path}\"]\n"));
        }
    }

    let mut sorted_roots = roots;
    sorted_roots.sort();
    for root in &sorted_roots {
        write_node(&mut out, root, &children_map, 1);
    }

    // Edges
    for edge in &dep_edges {
        if let (Some(&src), Some(&tgt)) = (
            id_to_path.get(edge.source.as_str()),
            id_to_path.get(edge.target.as_str()),
        ) {
            let kind_str = serde_json::to_string(&edge.kind).unwrap_or_default();
            let kind_label = kind_str.trim_matches('"');
            out.push_str(&format!(
                "    {} -->|{}| {}\n",
                mermaid_id(src),
                kind_label,
                mermaid_id(tgt)
            ));
        }
    }

    Ok(out)
}
```

**Step 2: Run tests**

Run: `cargo test -p svt-core mermaid -- --nocapture`
Expected: First 2 tests PASS. Snapshot test may fail (first run creates snapshot).

**Step 3: Accept snapshot if output looks correct**

Run: `cargo insta review` or `cargo insta accept`
Verify the snapshot content is a valid Mermaid flowchart.

**Step 4: Run all tests to confirm nothing broke**

Run: `cargo test -p svt-core`
Expected: All tests PASS.

**Step 5: Commit**

```bash
git add crates/core/src/export/ crates/core/src/lib.rs crates/core/src/snapshots/
git commit -m "feat(core): add Mermaid export module"
```

---

### Task 9: CLI Export Command

**Files:**
- Modify: `crates/cli/src/main.rs` (add Export variant, ExportArgs, run_export function)

**Step 1: Write CLI integration tests for export**

Add to `crates/cli/tests/cli_integration.rs`:

```rust
#[test]
fn export_mermaid_produces_flowchart() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Export mermaid
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .assert()
        .success()
        .stdout(predicate::str::contains("flowchart TD"))
        .stdout(predicate::str::contains("subgraph"));
}

#[test]
fn export_json_produces_valid_json() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Export JSON
    let output = svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("output should be valid JSON");
    assert_eq!(parsed["format"], "svt/v1");
}

#[test]
fn export_to_file_creates_output() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");
    let output_path = dir.path().join("output.mmd");

    // Import first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Export to file
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.starts_with("flowchart TD"));
}

#[test]
fn export_without_format_gives_error() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .assert()
        .failure();
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-cli export -- --nocapture`
Expected: FAIL — `export` subcommand does not exist yet.

**Step 3: Add Export subcommand to CLI**

In `crates/cli/src/main.rs`, add to `Commands` enum:

```rust
/// Export graph as Mermaid or JSON.
Export(ExportArgs),
```

Add args struct:

```rust
#[derive(clap::Args, Debug)]
struct ExportArgs {
    /// Output format: mermaid or json.
    #[arg(long)]
    format: String,

    /// Snapshot version to export (default: latest design).
    #[arg(long)]
    version: Option<u64>,

    /// Output file path (default: stdout).
    #[arg(long, short)]
    output: Option<PathBuf>,
}
```

Add `run_export` function:

```rust
fn run_export(store_path: &Path, args: &ExportArgs) -> Result<()> {
    use svt_core::model::SnapshotKind;

    let store = open_store(store_path)?;

    let version = match args.version {
        Some(v) => v,
        None => store
            .latest_version(SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let content = match args.format.as_str() {
        "mermaid" => svt_core::export::mermaid::to_mermaid(&store, version)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        "json" => svt_core::interchange_store::export_json(&store, version)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        other => bail!("Unsupported format: {other}. Use 'mermaid' or 'json'."),
    };

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &content)
            .with_context(|| format!("writing to {}", output_path.display()))?;
        println!("Exported to {}", output_path.display());
    } else {
        print!("{content}");
    }

    Ok(())
}
```

Add to `main` match:

```rust
Commands::Export(args) => run_export(&cli.store, args),
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-cli export -- --nocapture`
Expected: All 4 export tests PASS.

**Step 5: Commit**

```bash
git add crates/cli/src/main.rs crates/cli/tests/cli_integration.rs
git commit -m "feat(cli): add svt export command (mermaid + json)"
```

---

### Task 10: Update Dog-food Tests

**Files:**
- Modify: `crates/core/tests/dogfood.rs`

**Step 1: Update dogfood test to assert zero NotEvaluable**

Replace the `dogfood_conformance_all_must_not_depend_pass` test:

```rust
#[test]
fn dogfood_conformance_all_constraints_evaluated() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let report = conformance::evaluate_design(&store, version).unwrap();

    // All must_not_depend constraints should pass
    let must_not_depend_results: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.constraint_kind == "must_not_depend")
        .collect();

    assert!(
        !must_not_depend_results.is_empty(),
        "should have must_not_depend constraints"
    );

    for result in &must_not_depend_results {
        assert_eq!(
            result.status,
            ConstraintStatus::Pass,
            "constraint '{}' should pass but got {:?} with violations: {:?}",
            result.constraint_name,
            result.status,
            result.violations
        );
    }

    // No constraints should be NotEvaluable — all types are now implemented
    let not_evaluable: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.status == ConstraintStatus::NotEvaluable)
        .collect();

    assert!(
        not_evaluable.is_empty(),
        "all constraints should be evaluable, but these are not: {:?}",
        not_evaluable.iter().map(|r| &r.constraint_name).collect::<Vec<_>>()
    );

    // No error-level failures
    assert_eq!(report.summary.failed, 0, "no error-level failures expected");
}
```

**Step 2: Run the updated dogfood test**

Run: `cargo test -p svt-core dogfood -- --nocapture`
Expected: PASS — all 10 constraints now evaluate (no `NotEvaluable`).

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests PASS. Test count increases by ~13 new tests.

**Step 4: Commit**

```bash
git add crates/core/tests/dogfood.rs
git commit -m "test(core): update dogfood to assert all constraints evaluable"
```

---

### Task 11: Final Verification

**Step 1: Run clippy**

Run: `cargo clippy --all`
Expected: No warnings.

**Step 2: Run fmt check**

Run: `cargo fmt --check`
Expected: Clean.

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass. Total should be ~233+ (220 baseline + 13 new).

**Step 4: Manual smoke test**

Run: `cargo run --bin svt -- export --format mermaid --store .svt/store` (after importing architecture.yaml)
Verify: Output is a valid Mermaid flowchart with subgraphs and edges.

**Step 5: Update PROGRESS.md and commit**

Update `docs/plan/PROGRESS.md` to mark Milestone 6 as complete with test count and date.

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 6 as complete"
```
