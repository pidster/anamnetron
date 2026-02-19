# Milestone 12: DOT/Graphviz Export Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add DOT (Graphviz) export format through the existing `ExportRegistry`, enabling `svt export --format dot`.

**Architecture:** Implement a `DotExporter` struct that implements the `ExportFormat` trait (just like the existing `MermaidExporter`). DOT output uses `subgraph cluster_*` for containment hierarchy and directed edges with labels for non-containment relationships. Register it in `ExportRegistry::with_defaults()`. The CLI and server automatically pick it up through the registry — no CLI changes needed.

**Tech Stack:** Rust, Graphviz DOT language, `ExportFormat` trait, `insta` for snapshot testing.

---

### Task 1: Create DOT exporter module with basic graph output

**Files:**
- Create: `crates/core/src/export/dot.rs`
- Modify: `crates/core/src/export/mod.rs`

**Step 1: Write the failing test**

Add the `dot` module declaration and create the test file. In `crates/core/src/export/dot.rs`:

```rust
//! DOT (Graphviz) export.

use crate::model::*;
use crate::store::{GraphStore, Result};

/// Generate a DOT digraph from a graph store version.
///
/// Containment hierarchy is expressed via `subgraph cluster_*` blocks.
/// Non-containment edges are rendered as labelled arrows.
pub fn to_dot(store: &dyn GraphStore, version: Version) -> Result<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use crate::interchange::parse_yaml;
    use crate::interchange_store::load_into_store;
    use crate::store::CozoStore;

    #[test]
    fn simple_graph_produces_valid_dot() {
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

        let output = super::to_dot(&store, version).unwrap();

        assert!(output.starts_with("digraph"), "should start with digraph");
        assert!(output.contains("subgraph cluster_"), "should use cluster subgraphs");
        assert!(output.contains("depends"), "should contain edge label");
    }
}
```

In `crates/core/src/export/mod.rs`, add after the `pub mod mermaid;` line:

```rust
pub mod dot;
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export::dot::tests::simple_graph_produces_valid_dot`
Expected: FAIL with `not yet implemented`

**Step 3: Implement the DOT exporter**

Replace `todo!()` in `to_dot` with the full implementation. The logic mirrors `mermaid.rs` closely:

```rust
/// Sanitise a canonical path into a valid DOT node ID.
fn dot_id(path: &str) -> String {
    path.trim_start_matches('/').replace(['/', '-'], "_")
}

pub fn to_dot(store: &dyn GraphStore, version: Version) -> Result<String> {
    let nodes = store.get_all_nodes(version)?;
    let edges = store.get_all_edges(version, None)?;

    // Build ID-to-path mapping
    let id_to_path: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.canonical_path.as_str()))
        .collect();

    // Build parent map from Contains edges
    let mut parent_map: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for edge in &edges {
        if edge.kind == EdgeKind::Contains
            && id_to_path.contains_key(edge.source.as_str())
            && id_to_path.contains_key(edge.target.as_str())
        {
            parent_map.insert(
                id_to_path[edge.target.as_str()],
                id_to_path[edge.source.as_str()],
            );
        }
    }

    // Find root nodes (no parent)
    let node_paths: Vec<&str> = nodes.iter().map(|n| n.canonical_path.as_str()).collect();
    let mut roots: Vec<&str> = node_paths
        .iter()
        .filter(|p| !parent_map.contains_key(*p))
        .copied()
        .collect();
    roots.sort();

    // Build children map
    let mut children_map: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for (&child, &parent) in &parent_map {
        children_map.entry(parent).or_default().push(child);
    }
    for children in children_map.values_mut() {
        children.sort();
    }

    // Collect and sort non-containment edges
    let mut dep_edges: Vec<_> = edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Contains)
        .collect();
    dep_edges.sort_by(|a, b| {
        let a_src = id_to_path.get(a.source.as_str()).unwrap_or(&"");
        let b_src = id_to_path.get(b.source.as_str()).unwrap_or(&"");
        a_src
            .cmp(b_src)
            .then_with(|| {
                let a_tgt = id_to_path.get(a.target.as_str()).unwrap_or(&"");
                let b_tgt = id_to_path.get(b.target.as_str()).unwrap_or(&"");
                a_tgt.cmp(b_tgt)
            })
            .then_with(|| {
                let a_kind = serde_json::to_string(&a.kind).unwrap_or_default();
                let b_kind = serde_json::to_string(&b.kind).unwrap_or_default();
                a_kind.cmp(&b_kind)
            })
    });

    let mut out = String::new();
    out.push_str("digraph architecture {\n");
    out.push_str("    rankdir=TB;\n");
    out.push_str("    node [shape=box, style=filled, fillcolor=lightblue];\n");

    fn write_node(
        out: &mut String,
        path: &str,
        children_map: &std::collections::HashMap<&str, Vec<&str>>,
        indent: usize,
    ) {
        let pad = "    ".repeat(indent);
        let id = dot_id(path);

        if let Some(children) = children_map.get(path) {
            out.push_str(&format!("{pad}subgraph cluster_{id} {{\n"));
            out.push_str(&format!("{pad}    label=\"{path}\";\n"));
            for child in children {
                write_node(out, child, children_map, indent + 1);
            }
            out.push_str(&format!("{pad}}}\n"));
        } else {
            out.push_str(&format!("{pad}{id} [label=\"{path}\"];\n"));
        }
    }

    for root in &roots {
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
                "    {} -> {} [label=\"{}\"];\n",
                dot_id(src),
                dot_id(tgt),
                kind_label
            ));
        }
    }

    out.push_str("}\n");
    Ok(out)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p svt-core export::dot::tests::simple_graph_produces_valid_dot`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/export/dot.rs crates/core/src/export/mod.rs
git commit -m "feat(core): add DOT (Graphviz) export format"
```

---

### Task 2: Add comprehensive tests and snapshot test for DOT exporter

**Files:**
- Modify: `crates/core/src/export/dot.rs`

**Step 1: Write the additional tests**

Add these tests to the `mod tests` block in `dot.rs`:

```rust
    #[test]
    fn dot_contains_all_non_containment_edges() {
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

        let output = super::to_dot(&store, version).unwrap();

        assert!(output.contains("depends"), "should contain depends edge");
        assert!(output.contains("data_flow"), "should contain data_flow edge");
    }

    #[test]
    fn dot_snapshot_test() {
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

        let output = super::to_dot(&store, version).unwrap();
        insta::assert_snapshot!(output);
    }
```

**Step 2: Run tests to verify they pass**

Run: `cargo test -p svt-core export::dot::tests`
Expected: 3 tests pass. The snapshot test will create a new `.snap` file (auto-accepted on first run with `insta`).

**Step 3: Commit**

```bash
git add crates/core/src/export/dot.rs crates/core/src/export/snapshots/
git commit -m "test(core): add DOT exporter tests and snapshot"
```

---

### Task 3: Register DotExporter in ExportRegistry

**Files:**
- Modify: `crates/core/src/export/mod.rs`

**Step 1: Write the failing test**

Update the existing test in `crates/core/src/export/mod.rs` to expect "dot" in the registry:

Change the `export_registry_with_defaults_has_all_built_ins` test assertion from:
```rust
        assert_eq!(names, vec!["json", "mermaid"]);
```
to:
```rust
        assert_eq!(names, vec!["dot", "json", "mermaid"]);
```

Also add:
```rust
        assert!(registry.get("dot").is_some());
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export::tests::export_registry_with_defaults_has_all_built_ins`
Expected: FAIL — "dot" not registered yet

**Step 3: Add DotExporter struct and register it**

In `crates/core/src/export/mod.rs`, add after the `JsonExporter` impl block:

```rust
/// Built-in DOT (Graphviz) exporter.
#[derive(Debug)]
pub struct DotExporter;

impl ExportFormat for DotExporter {
    fn name(&self) -> &str {
        "dot"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        dot::to_dot(store, version)
    }
}
```

In `with_defaults()`, add after the `JsonExporter` registration:
```rust
        registry.register(Box::new(DotExporter));
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p svt-core export::tests`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/core/src/export/mod.rs
git commit -m "feat(core): register DotExporter in ExportRegistry"
```

---

### Task 4: Add CLI integration test for DOT export

**Files:**
- Modify: `crates/cli/tests/cli_integration.rs`

**Step 1: Write the failing test**

Add a new test to `crates/cli/tests/cli_integration.rs`:

```rust
#[test]
fn export_dot_produces_digraph() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join("store");

    // Import a design first
    Command::cargo_bin("svt")
        .unwrap()
        .args(["--store", store_path.to_str().unwrap(), "import"])
        .arg(design_yaml_path())
        .assert()
        .success();

    // Export as DOT
    let output = Command::cargo_bin("svt")
        .unwrap()
        .args([
            "--store",
            store_path.to_str().unwrap(),
            "export",
            "--format",
            "dot",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.starts_with("digraph"), "DOT output should start with digraph");
    assert!(stdout.contains("subgraph cluster_"), "DOT output should contain cluster subgraphs");
}
```

**Note:** Check how existing tests reference the design YAML file (look at `design_yaml_path()` or however the existing `export_mermaid_produces_flowchart` test works) and follow the same pattern.

**Step 2: Run test to verify it passes**

Run: `cargo test -p svt-cli export_dot_produces_digraph`
Expected: PASS (since DOT is already registered in the ExportRegistry, the CLI picks it up automatically)

**Step 3: Commit**

```bash
git add crates/cli/tests/cli_integration.rs
git commit -m "test(cli): add integration test for DOT export"
```

---

### Task 5: Update ExportArgs help text and run full verification

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Update the CLI help text**

In `crates/cli/src/main.rs`, change the doc comment on `ExportArgs.format` from:
```rust
    /// Output format: mermaid or json.
```
to:
```rust
    /// Output format: mermaid, json, or dot.
```

Also update the `Export` variant doc comment from:
```rust
    /// Export graph as Mermaid or JSON.
```
to:
```rust
    /// Export graph as Mermaid, JSON, or DOT.
```

**Step 2: Run full verification**

```bash
cargo fmt --check
cargo clippy
cargo test
cargo audit
cd crates/wasm && wasm-pack build --target web
cd ../../web && npm test
```

Expected: All clean, all tests pass.

**Step 3: Update PROGRESS.md**

Add M12 to the completed milestones table and update the current state line.

**Step 4: Commit**

```bash
git add crates/cli/src/main.rs docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 12 as complete, update CLI help text"
```
