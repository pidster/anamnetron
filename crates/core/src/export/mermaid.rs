//! Mermaid flowchart export.

use crate::model::*;
use crate::store::{GraphStore, Result};

/// Sanitise a canonical path into a valid Mermaid node ID.
fn mermaid_id(path: &str) -> String {
    path.trim_start_matches('/').replace(['/', '-'], "_")
}

/// Generate a Mermaid flowchart from a graph store version.
///
/// Containment hierarchy is expressed via `subgraph` blocks.
/// Non-containment edges are rendered as labelled arrows.
pub fn to_mermaid(store: &impl GraphStore, version: Version) -> Result<String> {
    let nodes = store.get_all_nodes(version)?;
    let edges = store.get_all_edges(version, None)?;

    // Build ID-to-path mapping from nodes
    let id_to_path: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.canonical_path.as_str()))
        .collect();

    // Build parent map from Contains edges
    let mut parent_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for edge in &edges {
        if edge.kind == EdgeKind::Contains {
            if let (Some(&_src_path), Some(&target_path)) = (
                id_to_path.get(edge.source.as_str()),
                id_to_path.get(edge.target.as_str()),
            ) {
                let _ = target_path; // used via insert below
                parent_map.insert(
                    id_to_path[edge.target.as_str()],
                    id_to_path[edge.source.as_str()],
                );
            }
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
    // Sort children for deterministic output
    for children in children_map.values_mut() {
        children.sort();
    }

    // Collect and sort non-containment edges for deterministic output
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
                "    {} -->|{}| {}\n",
                mermaid_id(src),
                kind_label,
                mermaid_id(tgt)
            ));
        }
    }

    Ok(out)
}

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
        assert!(
            output.contains("data_flow"),
            "should contain data_flow edge"
        );
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
