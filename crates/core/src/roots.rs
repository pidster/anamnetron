//! Root detection: identify entry points and terminal nodes from graph topology.
//!
//! Computes [`RootAnalysis`] by examining edge statistics across all nodes in a
//! snapshot. This is a pure topological analysis — no framework-specific patterns
//! or naming conventions are used.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::store::{GraphStore, Result};

/// Minimum ratio of in-degree to (out-degree + 1) for dependency source/sink classification.
///
/// A node qualifies as a dependency source when its `Depends` in-degree divided by
/// `(Depends out-degree + 1)` meets or exceeds this threshold (and vice-versa for sinks).
const DEPENDENCY_RATIO_THRESHOLD: f64 = 3.0;

/// An identified root node with display information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootEntry {
    /// The node's unique identifier.
    pub node_id: NodeId,
    /// The node's canonical path in the containment hierarchy.
    pub canonical_path: String,
    /// Human-readable display name.
    pub name: String,
}

/// Result of topological root detection across a snapshot.
///
/// Each category captures a different kind of "root" or "terminal" node
/// in the architecture graph. A single node may appear in multiple categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[must_use]
pub struct RootAnalysis {
    /// Nodes with outgoing `Calls` edges but zero incoming `Calls` edges.
    /// Typically: `main()`, top-level test functions, CLI entry points.
    pub call_tree_roots: Vec<RootEntry>,
    /// Nodes depended on by many but depending on few (`Depends` edges).
    /// Typically: shared libraries, core configuration modules.
    pub dependency_sources: Vec<RootEntry>,
    /// Nodes that depend on many but nothing depends on them (`Depends` edges).
    /// Typically: application entry modules, CLI commands.
    pub dependency_sinks: Vec<RootEntry>,
    /// `System`/`Service` nodes with no incoming `Contains` edges.
    /// Typically: workspace roots, top-level crates/packages.
    pub containment_roots: Vec<RootEntry>,
    /// Nodes with only incoming edges (no outgoing edges of any kind).
    /// Typically: database adapters, output formatters, leaf consumers.
    pub leaf_sinks: Vec<RootEntry>,
}

/// Node metadata collected from the store for classification.
struct NodeInfo {
    kind: NodeKind,
    canonical_path: String,
    name: String,
}

/// Per-node edge statistics accumulated during the single edge pass.
#[derive(Default)]
struct EdgeStats {
    calls_in: usize,
    calls_out: usize,
    depends_in: usize,
    depends_out: usize,
    is_contained: bool,
    has_any_outgoing: bool,
    has_any_incoming: bool,
}

/// Detect root and terminal nodes from graph topology.
///
/// Performs a single pass over all nodes and edges in the given snapshot version,
/// classifying nodes into five categories based on their edge patterns.
///
/// This function depends only on [`GraphStore`] trait methods and is WASM-compatible.
///
/// # Errors
///
/// Returns a store error if node or edge queries fail.
pub fn detect_roots(store: &(impl GraphStore + ?Sized), version: Version) -> Result<RootAnalysis> {
    // Collect all nodes with their metadata
    let all_nodes = store.get_all_nodes(version)?;
    let mut node_info: HashMap<&str, NodeInfo> = HashMap::with_capacity(all_nodes.len());
    for node in &all_nodes {
        node_info.insert(
            &node.id,
            NodeInfo {
                kind: node.kind,
                canonical_path: node.canonical_path.clone(),
                name: node.name.clone(),
            },
        );
    }

    // Accumulate edge statistics in a single pass
    let all_edges = store.get_all_edges(version, None)?;
    let mut stats: HashMap<&str, EdgeStats> = HashMap::new();

    for edge in &all_edges {
        let source_stats = stats.entry(&edge.source).or_default();
        source_stats.has_any_outgoing = true;
        match edge.kind {
            EdgeKind::Calls => source_stats.calls_out += 1,
            EdgeKind::Depends => source_stats.depends_out += 1,
            _ => {}
        }

        let target_stats = stats.entry(&edge.target).or_default();
        target_stats.has_any_incoming = true;
        match edge.kind {
            EdgeKind::Calls => target_stats.calls_in += 1,
            EdgeKind::Depends => target_stats.depends_in += 1,
            EdgeKind::Contains => target_stats.is_contained = true,
            _ => {}
        }
    }

    // Classify nodes into categories
    let mut call_tree_roots = Vec::new();
    let mut dependency_sources = Vec::new();
    let mut dependency_sinks = Vec::new();
    let mut containment_roots = Vec::new();
    let mut leaf_sinks = Vec::new();

    for (node_id, info) in &node_info {
        let s = stats.get(node_id).map(|s| s as &EdgeStats);
        let empty = EdgeStats::default();
        let s = s.unwrap_or(&empty);

        // Call-tree roots: outgoing Calls but no incoming Calls
        if s.calls_out > 0 && s.calls_in == 0 {
            call_tree_roots.push(make_entry(node_id, info));
        }

        // Dependency sources: high in-degree / low out-degree on Depends edges
        if s.depends_in > 0 {
            let ratio = s.depends_in as f64 / (s.depends_out as f64 + 1.0);
            if ratio >= DEPENDENCY_RATIO_THRESHOLD {
                dependency_sources.push(make_entry(node_id, info));
            }
        }

        // Dependency sinks: high out-degree / low in-degree on Depends edges
        if s.depends_out > 0 {
            let ratio = s.depends_out as f64 / (s.depends_in as f64 + 1.0);
            if ratio >= DEPENDENCY_RATIO_THRESHOLD {
                dependency_sinks.push(make_entry(node_id, info));
            }
        }

        // Containment roots: System/Service nodes not contained by anything
        if matches!(info.kind, NodeKind::System | NodeKind::Service) && !s.is_contained {
            containment_roots.push(make_entry(node_id, info));
        }

        // Leaf sinks: nodes with only incoming edges
        if s.has_any_incoming && !s.has_any_outgoing {
            leaf_sinks.push(make_entry(node_id, info));
        }
    }

    // Sort for deterministic output
    call_tree_roots.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    dependency_sources.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    dependency_sinks.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    containment_roots.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    leaf_sinks.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    Ok(RootAnalysis {
        call_tree_roots,
        dependency_sources,
        dependency_sinks,
        containment_roots,
        leaf_sinks,
    })
}

fn make_entry(node_id: &str, info: &NodeInfo) -> RootEntry {
    RootEntry {
        node_id: node_id.to_string(),
        canonical_path: info.canonical_path.clone(),
        name: info.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Edge, EdgeKind, Node, NodeKind, Provenance, SnapshotKind, DEFAULT_PROJECT_ID,
    };
    use crate::store::{CozoStore, GraphStore};

    fn make_node(id: &str, path: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            canonical_path: path.to_string(),
            qualified_name: None,
            kind,
            sub_kind: "test".to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            language: None,
            provenance: Provenance::Analysis,
            source_ref: None,
            metadata: None,
        }
    }

    fn make_edge(id: &str, source: &str, target: &str, kind: EdgeKind) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            kind,
            provenance: Provenance::Analysis,
            metadata: None,
        }
    }

    fn setup_store() -> (CozoStore, Version) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
            .unwrap();
        (store, v)
    }

    #[test]
    fn empty_graph_returns_empty_analysis() {
        let (store, v) = setup_store();
        let result = detect_roots(&store, v).unwrap();
        assert!(result.call_tree_roots.is_empty());
        assert!(result.dependency_sources.is_empty());
        assert!(result.dependency_sinks.is_empty());
        assert!(result.containment_roots.is_empty());
        assert!(result.leaf_sinks.is_empty());
    }

    #[test]
    fn single_node_no_edges_not_classified() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("n1", "/app/foo", NodeKind::Unit))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert!(result.call_tree_roots.is_empty());
        assert!(result.dependency_sources.is_empty());
        assert!(result.dependency_sinks.is_empty());
        assert!(result.containment_roots.is_empty());
        assert!(result.leaf_sinks.is_empty());
    }

    #[test]
    fn call_tree_root_detected() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("a", "/app/a", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("b", "/app/b", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("c", "/app/c", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "a", "b", EdgeKind::Calls))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "b", "c", EdgeKind::Calls))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert_eq!(result.call_tree_roots.len(), 1);
        assert_eq!(result.call_tree_roots[0].node_id, "a");
        assert_eq!(result.call_tree_roots[0].canonical_path, "/app/a");
        assert_eq!(result.call_tree_roots[0].name, "a");
    }

    #[test]
    fn mutual_calls_not_root() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("a", "/app/a", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("b", "/app/b", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "a", "b", EdgeKind::Calls))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "b", "a", EdgeKind::Calls))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert!(
            result.call_tree_roots.is_empty(),
            "mutual callers should not be call-tree roots"
        );
    }

    #[test]
    fn dependency_source_high_in_degree() {
        let (mut store, v) = setup_store();
        // "lib" is depended on by 4 others, depends on nothing
        store
            .add_node(v, &make_node("lib", "/app/lib", NodeKind::Component))
            .unwrap();
        for i in 0..4 {
            let id = format!("c{i}");
            let path = format!("/app/{id}");
            store
                .add_node(v, &make_node(&id, &path, NodeKind::Unit))
                .unwrap();
            store
                .add_edge(
                    v,
                    &make_edge(&format!("e{i}"), &id, "lib", EdgeKind::Depends),
                )
                .unwrap();
        }

        let result = detect_roots(&store, v).unwrap();
        assert_eq!(result.dependency_sources.len(), 1);
        assert_eq!(result.dependency_sources[0].node_id, "lib");
    }

    #[test]
    fn dependency_sink_high_out_degree() {
        let (mut store, v) = setup_store();
        // "app" depends on 4 others, nothing depends on it
        store
            .add_node(v, &make_node("app", "/app", NodeKind::Unit))
            .unwrap();
        for i in 0..4 {
            let id = format!("dep{i}");
            let path = format!("/deps/{id}");
            store
                .add_node(v, &make_node(&id, &path, NodeKind::Unit))
                .unwrap();
            store
                .add_edge(
                    v,
                    &make_edge(&format!("e{i}"), "app", &id, EdgeKind::Depends),
                )
                .unwrap();
        }

        let result = detect_roots(&store, v).unwrap();
        assert_eq!(result.dependency_sinks.len(), 1);
        assert_eq!(result.dependency_sinks[0].node_id, "app");
    }

    #[test]
    fn dependency_below_threshold_not_classified() {
        let (mut store, v) = setup_store();
        // "a" depends on 2 things, 1 thing depends on "a" -> ratio = 2/2 = 1.0 < 3.0
        store
            .add_node(v, &make_node("a", "/app/a", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("b", "/app/b", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("c", "/app/c", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "a", "b", EdgeKind::Depends))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "a", "c", EdgeKind::Depends))
            .unwrap();
        store
            .add_edge(v, &make_edge("e3", "b", "a", EdgeKind::Depends))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert!(
            result.dependency_sinks.is_empty(),
            "ratio below threshold should not classify as sink"
        );
    }

    #[test]
    fn containment_root_system_node() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("sys", "/workspace", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("svc", "/workspace/svc", NodeKind::Service))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "sys", "svc", EdgeKind::Contains))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        // sys is a System node not contained by anything -> containment root
        assert_eq!(result.containment_roots.len(), 1);
        assert_eq!(result.containment_roots[0].node_id, "sys");
    }

    #[test]
    fn contained_system_node_not_root() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("outer", "/outer", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("inner", "/outer/inner", NodeKind::System))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "outer", "inner", EdgeKind::Contains))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        // outer is root, inner is contained
        assert_eq!(result.containment_roots.len(), 1);
        assert_eq!(result.containment_roots[0].node_id, "outer");
    }

    #[test]
    fn component_node_not_containment_root() {
        let (mut store, v) = setup_store();
        // Component nodes are never containment roots, even if not contained
        store
            .add_node(v, &make_node("comp", "/app/comp", NodeKind::Component))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert!(
            result.containment_roots.is_empty(),
            "Component nodes should not be containment roots"
        );
    }

    #[test]
    fn leaf_sink_only_incoming() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("a", "/app/a", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("b", "/app/b", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "a", "b", EdgeKind::Calls))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        assert_eq!(result.leaf_sinks.len(), 1);
        assert_eq!(result.leaf_sinks[0].node_id, "b");
    }

    #[test]
    fn node_with_both_directions_not_leaf() {
        let (mut store, v) = setup_store();
        store
            .add_node(v, &make_node("a", "/app/a", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("b", "/app/b", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("c", "/app/c", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e1", "a", "b", EdgeKind::Calls))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "b", "c", EdgeKind::Calls))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();
        // b has both incoming and outgoing -> not a leaf sink
        let leaf_ids: Vec<&str> = result
            .leaf_sinks
            .iter()
            .map(|e| e.node_id.as_str())
            .collect();
        assert!(!leaf_ids.contains(&"b"), "b should not be a leaf sink");
        assert!(leaf_ids.contains(&"c"), "c should be a leaf sink");
    }

    #[test]
    fn mixed_graph_all_categories() {
        let (mut store, v) = setup_store();

        // System node (containment root)
        store
            .add_node(v, &make_node("sys", "/sys", NodeKind::System))
            .unwrap();

        // Call-tree root: main calls handler
        store
            .add_node(v, &make_node("main", "/sys/main", NodeKind::Unit))
            .unwrap();
        store
            .add_node(v, &make_node("handler", "/sys/handler", NodeKind::Unit))
            .unwrap();
        store
            .add_edge(v, &make_edge("e-call", "main", "handler", EdgeKind::Calls))
            .unwrap();

        // Dependency source: lib depended on by 3 others
        store
            .add_node(v, &make_node("lib", "/sys/lib", NodeKind::Component))
            .unwrap();
        for i in 0..3 {
            let id = format!("user{i}");
            let path = format!("/sys/{id}");
            store
                .add_node(v, &make_node(&id, &path, NodeKind::Unit))
                .unwrap();
            store
                .add_edge(
                    v,
                    &make_edge(&format!("e-dep{i}"), &id, "lib", EdgeKind::Depends),
                )
                .unwrap();
        }

        // Leaf sink: handler has incoming calls, no outgoing
        // (already added above — handler gets called by main, calls nothing)

        // Containment
        store
            .add_edge(v, &make_edge("e-c1", "sys", "main", EdgeKind::Contains))
            .unwrap();

        let result = detect_roots(&store, v).unwrap();

        assert_eq!(
            result.call_tree_roots.len(),
            1,
            "main should be call-tree root"
        );
        assert_eq!(result.call_tree_roots[0].node_id, "main");

        assert!(
            result.dependency_sources.iter().any(|e| e.node_id == "lib"),
            "lib should be a dependency source"
        );

        assert_eq!(
            result.containment_roots.len(),
            1,
            "sys should be containment root"
        );
        assert_eq!(result.containment_roots[0].node_id, "sys");

        assert!(
            result.leaf_sinks.iter().any(|e| e.node_id == "handler"),
            "handler should be a leaf sink"
        );
    }

    #[test]
    fn deterministic_ordering() {
        let (mut store, v) = setup_store();

        // Add nodes in reverse order to test sorting
        for id in ["z", "m", "a"] {
            let path = format!("/app/{id}");
            store
                .add_node(v, &make_node(id, &path, NodeKind::System))
                .unwrap();
        }

        let r1 = detect_roots(&store, v).unwrap();
        let r2 = detect_roots(&store, v).unwrap();
        assert_eq!(r1, r2, "repeated calls should produce identical results");

        // Containment roots should be sorted by node_id
        let ids: Vec<&str> = r1
            .containment_roots
            .iter()
            .map(|e| e.node_id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "m", "z"]);
    }
}
