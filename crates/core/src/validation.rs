//! Graph validation: structural invariants and referential integrity.

use std::collections::{HashMap, HashSet};

use crate::model::*;
use crate::store::{GraphStore, Result};

/// A cycle detected in the containment hierarchy.
#[derive(Debug, Clone)]
pub struct Cycle {
    /// Node IDs forming the cycle.
    pub node_ids: Vec<NodeId>,
}

/// A referential integrity error.
#[derive(Debug, Clone)]
pub struct IntegrityError {
    /// The edge with the invalid reference.
    pub edge_id: EdgeId,
    /// The missing node ID.
    pub missing_node_id: NodeId,
}

/// Check that contains edges form a DAG (no cycles).
///
/// Collects all `Contains` edges and performs DFS-based cycle detection.
/// Returns a list of detected cycles (empty if the graph is acyclic).
pub fn validate_contains_acyclic(store: &impl GraphStore, version: Version) -> Result<Vec<Cycle>> {
    let contains_edges = store.get_all_edges(version, Some(EdgeKind::Contains))?;

    // Build adjacency list: parent -> [children]
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_nodes: HashSet<String> = HashSet::new();

    for edge in &contains_edges {
        adj.entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        all_nodes.insert(edge.source.clone());
        all_nodes.insert(edge.target.clone());
    }

    // DFS cycle detection using three-color marking
    let mut white: HashSet<&str> = all_nodes.iter().map(|s| s.as_str()).collect();
    let mut gray: HashSet<&str> = HashSet::new();
    let mut cycles = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &'a HashMap<String, Vec<String>>,
        white: &mut HashSet<&'a str>,
        gray: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
        cycles: &mut Vec<Cycle>,
    ) {
        white.remove(node);
        gray.insert(node);
        path.push(node);

        if let Some(children) = adj.get(node) {
            for child in children {
                if gray.contains(child.as_str()) {
                    // Found a cycle: extract the cycle from path
                    let cycle_start = path.iter().position(|&n| n == child.as_str()).unwrap();
                    let cycle_nodes: Vec<NodeId> =
                        path[cycle_start..].iter().map(|s| s.to_string()).collect();
                    cycles.push(Cycle {
                        node_ids: cycle_nodes,
                    });
                } else if white.contains(child.as_str()) {
                    dfs(child.as_str(), adj, white, gray, path, cycles);
                }
            }
        }

        gray.remove(node);
        path.pop();
    }

    let nodes_vec: Vec<String> = all_nodes.iter().cloned().collect();
    for node in &nodes_vec {
        if white.contains(node.as_str()) {
            let mut path = Vec::new();
            dfs(
                node.as_str(),
                &adj,
                &mut white,
                &mut gray,
                &mut path,
                &mut cycles,
            );
        }
    }

    Ok(cycles)
}

/// Check that all edge source/target references point to existing nodes.
///
/// Queries all edges and verifies that both source and target node IDs
/// exist in the store for the given version.
pub fn validate_referential_integrity(
    store: &impl GraphStore,
    version: Version,
) -> Result<Vec<IntegrityError>> {
    let mut errors = Vec::new();

    // Get all edges of all kinds
    let all_edge_kinds = [
        EdgeKind::Contains,
        EdgeKind::Depends,
        EdgeKind::Calls,
        EdgeKind::Implements,
        EdgeKind::Extends,
        EdgeKind::DataFlow,
        EdgeKind::Exports,
    ];

    for kind in all_edge_kinds {
        let edges = store.get_all_edges(version, Some(kind))?;
        for edge in &edges {
            if store.get_node(version, &edge.source)?.is_none() {
                errors.push(IntegrityError {
                    edge_id: edge.id.clone(),
                    missing_node_id: edge.source.clone(),
                });
            }
            if store.get_node(version, &edge.target)?.is_none() {
                errors.push(IntegrityError {
                    edge_id: edge.id.clone(),
                    missing_node_id: edge.target.clone(),
                });
            }
        }
    }

    Ok(errors)
}
