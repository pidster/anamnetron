//! Snapshot diffing: compute changes between two graph versions.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::store::{GraphStore, Result};

/// How a node changed between two versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Present in `to` but not in `from`.
    Added,
    /// Present in `from` but not in `to`.
    Removed,
    /// Present in both but with different properties.
    Changed,
}

/// A node that differs between two snapshot versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeChange {
    /// The canonical path of the node.
    pub canonical_path: String,
    /// How the node changed.
    pub change: ChangeKind,
    /// The node kind (from whichever version has it).
    pub kind: NodeKind,
    /// The sub-kind (from whichever version has it).
    pub sub_kind: String,
    /// What fields changed (only populated for `Changed`).
    pub changed_fields: Vec<String>,
}

/// An edge that differs between two snapshot versions.
///
/// Edges are matched by (source canonical path, target canonical path, edge kind).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeChange {
    /// Source node canonical path.
    pub source_path: String,
    /// Target node canonical path.
    pub target_path: String,
    /// Edge kind.
    pub edge_kind: EdgeKind,
    /// How the edge changed.
    pub change: ChangeKind,
}

/// Summary counts for a snapshot diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Number of added nodes.
    pub nodes_added: usize,
    /// Number of removed nodes.
    pub nodes_removed: usize,
    /// Number of changed nodes.
    pub nodes_changed: usize,
    /// Number of added edges.
    pub edges_added: usize,
    /// Number of removed edges.
    pub edges_removed: usize,
}

/// The result of comparing two snapshot versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiff {
    /// The base version being compared from.
    pub from_version: Version,
    /// The target version being compared to.
    pub to_version: Version,
    /// Node-level changes.
    pub node_changes: Vec<NodeChange>,
    /// Edge-level changes.
    pub edge_changes: Vec<EdgeChange>,
    /// Summary counts.
    pub summary: DiffSummary,
}

/// Compare two snapshot versions and compute the diff.
///
/// Nodes are matched by canonical path. Edges are matched by
/// (source canonical path, target canonical path, edge kind).
pub fn diff_snapshots(
    store: &dyn GraphStore,
    from_version: Version,
    to_version: Version,
) -> Result<SnapshotDiff> {
    let from_nodes = store.get_all_nodes(from_version)?;
    let to_nodes = store.get_all_nodes(to_version)?;
    let from_edges = store.get_all_edges(from_version, None)?;
    let to_edges = store.get_all_edges(to_version, None)?;

    // Index nodes by canonical path
    let from_node_map: HashMap<&str, &Node> = from_nodes
        .iter()
        .map(|n| (n.canonical_path.as_str(), n))
        .collect();
    let to_node_map: HashMap<&str, &Node> = to_nodes
        .iter()
        .map(|n| (n.canonical_path.as_str(), n))
        .collect();

    // Diff nodes
    let mut node_changes = Vec::new();

    // Check for removed and changed nodes
    for (path, from_node) in &from_node_map {
        if let Some(to_node) = to_node_map.get(path) {
            let changed_fields = diff_node_fields(from_node, to_node);
            if !changed_fields.is_empty() {
                node_changes.push(NodeChange {
                    canonical_path: path.to_string(),
                    change: ChangeKind::Changed,
                    kind: to_node.kind,
                    sub_kind: to_node.sub_kind.clone(),
                    changed_fields,
                });
            }
        } else {
            node_changes.push(NodeChange {
                canonical_path: path.to_string(),
                change: ChangeKind::Removed,
                kind: from_node.kind,
                sub_kind: from_node.sub_kind.clone(),
                changed_fields: Vec::new(),
            });
        }
    }

    // Check for added nodes
    for (path, to_node) in &to_node_map {
        if !from_node_map.contains_key(path) {
            node_changes.push(NodeChange {
                canonical_path: path.to_string(),
                change: ChangeKind::Added,
                kind: to_node.kind,
                sub_kind: to_node.sub_kind.clone(),
                changed_fields: Vec::new(),
            });
        }
    }

    // Sort for deterministic output
    node_changes.sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));

    // Build node ID to canonical path maps for edge resolution
    let from_id_to_path: HashMap<&str, &str> = from_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.canonical_path.as_str()))
        .collect();
    let to_id_to_path: HashMap<&str, &str> = to_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.canonical_path.as_str()))
        .collect();

    // Resolve edges to (source_path, target_path, kind) tuples
    type EdgeKey = (String, String, EdgeKind);

    let from_edge_keys: HashSet<EdgeKey> = from_edges
        .iter()
        .filter_map(|e| {
            let src = from_id_to_path.get(e.source.as_str())?;
            let tgt = from_id_to_path.get(e.target.as_str())?;
            Some((src.to_string(), tgt.to_string(), e.kind))
        })
        .collect();

    let to_edge_keys: HashSet<EdgeKey> = to_edges
        .iter()
        .filter_map(|e| {
            let src = to_id_to_path.get(e.source.as_str())?;
            let tgt = to_id_to_path.get(e.target.as_str())?;
            Some((src.to_string(), tgt.to_string(), e.kind))
        })
        .collect();

    let mut edge_changes = Vec::new();

    for key in &from_edge_keys {
        if !to_edge_keys.contains(key) {
            edge_changes.push(EdgeChange {
                source_path: key.0.clone(),
                target_path: key.1.clone(),
                edge_kind: key.2,
                change: ChangeKind::Removed,
            });
        }
    }

    for key in &to_edge_keys {
        if !from_edge_keys.contains(key) {
            edge_changes.push(EdgeChange {
                source_path: key.0.clone(),
                target_path: key.1.clone(),
                edge_kind: key.2,
                change: ChangeKind::Added,
            });
        }
    }

    // Sort for deterministic output
    edge_changes.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then_with(|| a.target_path.cmp(&b.target_path))
            .then_with(|| {
                serde_json::to_string(&a.edge_kind)
                    .unwrap_or_default()
                    .cmp(&serde_json::to_string(&b.edge_kind).unwrap_or_default())
            })
    });

    let summary = DiffSummary {
        nodes_added: node_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Added)
            .count(),
        nodes_removed: node_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Removed)
            .count(),
        nodes_changed: node_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Changed)
            .count(),
        edges_added: edge_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Added)
            .count(),
        edges_removed: edge_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Removed)
            .count(),
    };

    Ok(SnapshotDiff {
        from_version,
        to_version,
        node_changes,
        edge_changes,
        summary,
    })
}

/// Compare two nodes and return which fields differ.
fn diff_node_fields(a: &Node, b: &Node) -> Vec<String> {
    let mut changed = Vec::new();
    if a.kind != b.kind {
        changed.push("kind".to_string());
    }
    if a.sub_kind != b.sub_kind {
        changed.push("sub_kind".to_string());
    }
    if a.language != b.language {
        changed.push("language".to_string());
    }
    if a.provenance != b.provenance {
        changed.push("provenance".to_string());
    }
    if a.source_ref != b.source_ref {
        changed.push("source_ref".to_string());
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::CozoStore;

    fn make_node(path: &str, kind: NodeKind, sub_kind: &str) -> Node {
        Node {
            id: uuid::Uuid::new_v4().to_string(),
            canonical_path: path.to_string(),
            qualified_name: None,
            kind,
            sub_kind: sub_kind.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            language: Some("rust".to_string()),
            provenance: Provenance::Analysis,
            source_ref: None,
            metadata: None,
        }
    }

    fn make_edge(source_id: &str, target_id: &str, kind: EdgeKind) -> Edge {
        Edge {
            id: uuid::Uuid::new_v4().to_string(),
            source: source_id.to_string(),
            target: target_id.to_string(),
            kind,
            provenance: Provenance::Analysis,
            metadata: None,
        }
    }

    #[test]
    fn identical_snapshots_produce_empty_diff() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let node = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v1, &node).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let node2 = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v2, &node2).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert!(diff.node_changes.is_empty(), "no node changes expected");
        assert!(diff.edge_changes.is_empty(), "no edge changes expected");
        assert_eq!(diff.summary.nodes_added, 0);
        assert_eq!(diff.summary.nodes_removed, 0);
        assert_eq!(diff.summary.nodes_changed, 0);
    }

    #[test]
    fn detects_added_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1 = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v1, &n1).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2a = make_node("/app/core", NodeKind::Service, "crate");
        let n2b = make_node("/app/cli", NodeKind::Service, "crate");
        store.add_nodes_batch(v2, &[n2a, n2b]).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert_eq!(diff.summary.nodes_added, 1);
        assert_eq!(diff.summary.nodes_removed, 0);
        assert_eq!(diff.node_changes[0].canonical_path, "/app/cli");
        assert_eq!(diff.node_changes[0].change, ChangeKind::Added);
    }

    #[test]
    fn detects_removed_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1a = make_node("/app/core", NodeKind::Service, "crate");
        let n1b = make_node("/app/cli", NodeKind::Service, "crate");
        store.add_nodes_batch(v1, &[n1a, n1b]).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2 = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v2, &n2).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert_eq!(diff.summary.nodes_removed, 1);
        assert_eq!(diff.summary.nodes_added, 0);
        let removed: Vec<_> = diff
            .node_changes
            .iter()
            .filter(|c| c.change == ChangeKind::Removed)
            .collect();
        assert_eq!(removed[0].canonical_path, "/app/cli");
    }

    #[test]
    fn detects_changed_node_kind() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1 = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v1, &n1).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2 = make_node("/app/core", NodeKind::Component, "module");
        store.add_node(v2, &n2).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert_eq!(diff.summary.nodes_changed, 1);
        let changed = &diff.node_changes[0];
        assert_eq!(changed.change, ChangeKind::Changed);
        assert!(changed.changed_fields.contains(&"kind".to_string()));
        assert!(changed.changed_fields.contains(&"sub_kind".to_string()));
    }

    #[test]
    fn detects_added_edges() {
        let mut store = CozoStore::new_in_memory().unwrap();

        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1a = make_node("/app/core", NodeKind::Service, "crate");
        let n1b = make_node("/app/cli", NodeKind::Service, "crate");
        store
            .add_nodes_batch(v1, &[n1a.clone(), n1b.clone()])
            .unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2a = make_node("/app/core", NodeKind::Service, "crate");
        let n2b = make_node("/app/cli", NodeKind::Service, "crate");
        store
            .add_nodes_batch(v2, &[n2a.clone(), n2b.clone()])
            .unwrap();
        let edge = make_edge(&n2b.id, &n2a.id, EdgeKind::Depends);
        store.add_edge(v2, &edge).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert_eq!(diff.summary.edges_added, 1);
        assert_eq!(diff.summary.edges_removed, 0);
        assert_eq!(diff.edge_changes[0].source_path, "/app/cli");
        assert_eq!(diff.edge_changes[0].target_path, "/app/core");
        assert_eq!(diff.edge_changes[0].change, ChangeKind::Added);
    }

    #[test]
    fn detects_removed_edges() {
        let mut store = CozoStore::new_in_memory().unwrap();

        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1a = make_node("/app/core", NodeKind::Service, "crate");
        let n1b = make_node("/app/cli", NodeKind::Service, "crate");
        store
            .add_nodes_batch(v1, &[n1a.clone(), n1b.clone()])
            .unwrap();
        let edge = make_edge(&n1b.id, &n1a.id, EdgeKind::Depends);
        store.add_edge(v1, &edge).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2a = make_node("/app/core", NodeKind::Service, "crate");
        let n2b = make_node("/app/cli", NodeKind::Service, "crate");
        store.add_nodes_batch(v2, &[n2a, n2b]).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert_eq!(diff.summary.edges_removed, 1);
        assert_eq!(diff.summary.edges_added, 0);
    }

    #[test]
    fn diff_symmetry_swaps_added_and_removed() {
        let mut store = CozoStore::new_in_memory().unwrap();

        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n1 = make_node("/app/core", NodeKind::Service, "crate");
        store.add_node(v1, &n1).unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2a = make_node("/app/core", NodeKind::Service, "crate");
        let n2b = make_node("/app/cli", NodeKind::Service, "crate");
        store.add_nodes_batch(v2, &[n2a, n2b]).unwrap();

        let forward = diff_snapshots(&store, v1, v2).unwrap();
        let backward = diff_snapshots(&store, v2, v1).unwrap();

        assert_eq!(forward.summary.nodes_added, backward.summary.nodes_removed);
        assert_eq!(forward.summary.nodes_removed, backward.summary.nodes_added);
    }

    #[test]
    fn empty_snapshots_produce_empty_diff() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();

        assert!(diff.node_changes.is_empty());
        assert!(diff.edge_changes.is_empty());
    }

    #[test]
    fn diff_serializes_to_json() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        let n2 = make_node("/app/new", NodeKind::Service, "crate");
        store.add_node(v2, &n2).unwrap();

        let diff = diff_snapshots(&store, v1, v2).unwrap();
        let json = serde_json::to_string_pretty(&diff).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("from_version").is_some());
        assert!(parsed.get("to_version").is_some());
        assert!(parsed.get("node_changes").is_some());
        assert!(parsed.get("edge_changes").is_some());
        assert!(parsed.get("summary").is_some());
    }
}
