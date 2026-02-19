//! WASM bridge to `svt-core` for browser-side graph queries.
//!
//! This crate exposes a `WasmStore` struct via `wasm_bindgen` that wraps
//! the CozoDB-backed `GraphStore` implementation, allowing the web frontend
//! to run graph queries entirely in the browser.

#![warn(missing_docs)]

use svt_core::model::{Direction, Edge, EdgeKind, Node, SnapshotKind};
use svt_core::store::{CozoStore, GraphStore};
use wasm_bindgen::prelude::*;

/// WASM-accessible graph store backed by an in-memory CozoDB instance.
#[wasm_bindgen]
pub struct WasmStore {
    store: CozoStore,
}

#[wasm_bindgen]
impl WasmStore {
    /// Create a new empty in-memory store.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmStore, JsError> {
        let store = CozoStore::new_in_memory().map_err(|e| JsError::new(&e.to_string()))?;
        Ok(WasmStore { store })
    }

    /// Load a snapshot from JSON arrays of nodes and edges.
    ///
    /// The JSON format matches the `svt_core::model::Node` and `Edge` serialization.
    /// Returns the new snapshot version number.
    pub fn load_snapshot(&mut self, nodes_json: &str, edges_json: &str) -> Result<u64, JsError> {
        let nodes: Vec<Node> =
            serde_json::from_str(nodes_json).map_err(|e| JsError::new(&e.to_string()))?;
        let edges: Vec<Edge> =
            serde_json::from_str(edges_json).map_err(|e| JsError::new(&e.to_string()))?;

        let version = self
            .store
            .create_snapshot(SnapshotKind::Import, None)
            .map_err(|e| JsError::new(&e.to_string()))?;

        self.store
            .add_nodes_batch(version, &nodes)
            .map_err(|e| JsError::new(&e.to_string()))?;

        self.store
            .add_edges_batch(version, &edges)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(version)
    }

    /// Get a node by its ID within a version. Returns node JSON or "null".
    pub fn get_node(&self, version: u64, id: &str) -> Result<String, JsError> {
        let result = self
            .store
            .get_node(version, &id.to_string())
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get a node by its canonical path within a version. Returns node JSON or "null".
    pub fn get_node_by_path(&self, version: u64, path: &str) -> Result<String, JsError> {
        let result = self
            .store
            .get_node_by_path(version, path)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get all nodes for a version. Returns a JSON array.
    pub fn get_all_nodes(&self, version: u64) -> Result<String, JsError> {
        let result = self
            .store
            .get_all_nodes(version)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get the direct children of a node. Returns a JSON array.
    pub fn get_children(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let result = self
            .store
            .get_children(version, &node_id.to_string())
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get the parent of a node. Returns node JSON or "null".
    pub fn get_parent(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let result = self
            .store
            .get_parent(version, &node_id.to_string())
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get all ancestors of a node. Returns a JSON array.
    pub fn get_ancestors(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let result = self
            .store
            .query_ancestors(version, &node_id.to_string())
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get all descendants of a node. Returns a JSON array.
    pub fn get_descendants(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let result = self
            .store
            .query_descendants(version, &node_id.to_string(), None)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get edges connected to a node.
    ///
    /// Direction must be `"outgoing"`, `"incoming"`, or `"both"`.
    /// Kind is an optional edge kind filter (e.g. `"depends"`, `"contains"`).
    /// Returns a JSON array of edges.
    pub fn get_edges(
        &self,
        version: u64,
        node_id: &str,
        direction: &str,
        kind: Option<String>,
    ) -> Result<String, JsError> {
        let dir = parse_direction(direction).map_err(|e| JsError::new(&e))?;
        let edge_kind = match kind {
            Some(ref k) => Some(parse_edge_kind(k).map_err(|e| JsError::new(&e))?),
            None => None,
        };

        let result = self
            .store
            .get_edges(version, &node_id.to_string(), dir, edge_kind)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get all edges for a version, optionally filtered by kind. Returns a JSON array.
    pub fn get_all_edges(&self, version: u64, kind: Option<String>) -> Result<String, JsError> {
        let edge_kind = match kind {
            Some(ref k) => Some(parse_edge_kind(k).map_err(|e| JsError::new(&e))?),
            None => None,
        };

        let result = self
            .store
            .get_all_edges(version, edge_kind)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get dependencies of a node (nodes it depends on).
    ///
    /// If `transitive` is true, follows the dependency chain recursively.
    /// Returns a JSON array of nodes.
    pub fn get_dependencies(
        &self,
        version: u64,
        node_id: &str,
        transitive: bool,
    ) -> Result<String, JsError> {
        let result = self
            .store
            .query_dependencies(version, &node_id.to_string(), transitive)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get dependents of a node (nodes that depend on it).
    ///
    /// If `transitive` is true, follows the dependent chain recursively.
    /// Returns a JSON array of nodes.
    pub fn get_dependents(
        &self,
        version: u64,
        node_id: &str,
        transitive: bool,
    ) -> Result<String, JsError> {
        let result = self
            .store
            .query_dependents(version, &node_id.to_string(), transitive)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Search for nodes whose canonical path matches a glob pattern.
    ///
    /// Uses `svt_core::canonical::canonical_path_matches` for matching.
    /// Returns a JSON array of matching nodes.
    pub fn search(&self, version: u64, pattern: &str) -> Result<String, JsError> {
        let all_nodes = self
            .store
            .get_all_nodes(version)
            .map_err(|e| JsError::new(&e.to_string()))?;

        let matching: Vec<&Node> = all_nodes
            .iter()
            .filter(|n| svt_core::canonical::canonical_path_matches(&n.canonical_path, pattern))
            .collect();

        serde_json::to_string(&matching).map_err(|e| JsError::new(&e.to_string()))
    }
}

/// Parse a direction string into a `Direction` enum value.
fn parse_direction(s: &str) -> Result<Direction, String> {
    match s {
        "outgoing" => Ok(Direction::Outgoing),
        "incoming" => Ok(Direction::Incoming),
        "both" => Ok(Direction::Both),
        _ => Err(format!(
            "invalid direction: '{s}' (expected 'outgoing', 'incoming', or 'both')"
        )),
    }
}

/// Parse an edge kind string into an `EdgeKind` enum value.
fn parse_edge_kind(s: &str) -> Result<EdgeKind, String> {
    serde_json::from_value::<EdgeKind>(serde_json::Value::String(s.to_string()))
        .map_err(|e| format!("invalid edge kind '{s}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::model::*;

    /// Create a test store with 3 nodes and 3 edges:
    /// - /app (system, n1), /app/core (service, n2), /app/cli (service, n3)
    /// - n1->n2 contains, n1->n3 contains, n3->n2 depends
    fn make_test_store() -> WasmStore {
        let mut store = WasmStore::new().expect("failed to create store");

        let nodes_json = serde_json::to_string(&vec![
            Node {
                id: "n1".to_string(),
                canonical_path: "/app".to_string(),
                qualified_name: None,
                kind: NodeKind::System,
                sub_kind: "workspace".to_string(),
                name: "app".to_string(),
                language: None,
                provenance: Provenance::Design,
                source_ref: None,
                metadata: None,
            },
            Node {
                id: "n2".to_string(),
                canonical_path: "/app/core".to_string(),
                qualified_name: None,
                kind: NodeKind::Service,
                sub_kind: "crate".to_string(),
                name: "core".to_string(),
                language: None,
                provenance: Provenance::Design,
                source_ref: None,
                metadata: None,
            },
            Node {
                id: "n3".to_string(),
                canonical_path: "/app/cli".to_string(),
                qualified_name: None,
                kind: NodeKind::Service,
                sub_kind: "crate".to_string(),
                name: "cli".to_string(),
                language: None,
                provenance: Provenance::Design,
                source_ref: None,
                metadata: None,
            },
        ])
        .unwrap();

        let edges_json = serde_json::to_string(&vec![
            Edge {
                id: "e1".to_string(),
                source: "n1".to_string(),
                target: "n2".to_string(),
                kind: EdgeKind::Contains,
                provenance: Provenance::Design,
                metadata: None,
            },
            Edge {
                id: "e2".to_string(),
                source: "n1".to_string(),
                target: "n3".to_string(),
                kind: EdgeKind::Contains,
                provenance: Provenance::Design,
                metadata: None,
            },
            Edge {
                id: "e3".to_string(),
                source: "n3".to_string(),
                target: "n2".to_string(),
                kind: EdgeKind::Depends,
                provenance: Provenance::Design,
                metadata: None,
            },
        ])
        .unwrap();

        let version = store
            .load_snapshot(&nodes_json, &edges_json)
            .expect("failed to load snapshot");
        assert_eq!(version, 1, "first snapshot should be version 1");

        store
    }

    #[test]
    fn wasm_store_new_succeeds() {
        let store = WasmStore::new().expect("store creation should succeed");
        let snapshots = store
            .store
            .list_snapshots()
            .expect("list_snapshots should succeed");
        assert!(snapshots.is_empty(), "new store should have no snapshots");
    }

    #[test]
    fn load_snapshot_creates_version() {
        let mut store = WasmStore::new().expect("store creation should succeed");
        let nodes_json = serde_json::to_string(&vec![Node {
            id: "n1".to_string(),
            canonical_path: "/svc".to_string(),
            qualified_name: None,
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            name: "svc".to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        }])
        .unwrap();

        let version = store
            .load_snapshot(&nodes_json, "[]")
            .expect("load_snapshot should succeed");
        assert_eq!(version, 1, "first snapshot should be version 1");

        let all_nodes = store
            .store
            .get_all_nodes(version)
            .expect("get_all_nodes should succeed");
        assert_eq!(all_nodes.len(), 1);
        assert_eq!(all_nodes[0].id, "n1");
    }

    #[test]
    fn load_snapshot_with_edges() {
        let store = make_test_store();

        let all_nodes = store
            .store
            .get_all_nodes(1)
            .expect("get_all_nodes should succeed");
        assert_eq!(all_nodes.len(), 3, "should have 3 nodes");

        let all_edges = store
            .store
            .get_all_edges(1, None)
            .expect("get_all_edges should succeed");
        assert_eq!(all_edges.len(), 3, "should have 3 edges");
    }

    // --- Read-only query method tests ---

    #[test]
    fn get_node_returns_correct_json() {
        let store = make_test_store();
        let json = store.get_node(1, "n1").expect("get_node should succeed");
        let node: Option<Node> = serde_json::from_str(&json).unwrap();
        let node = node.expect("node n1 should exist");
        assert_eq!(node.id, "n1");
        assert_eq!(node.canonical_path, "/app");
    }

    #[test]
    fn get_node_returns_null_for_missing() {
        let store = make_test_store();
        let json = store
            .get_node(1, "nonexistent")
            .expect("get_node should succeed");
        assert_eq!(json, "null");
    }

    #[test]
    fn get_node_by_path_returns_correct_json() {
        let store = make_test_store();
        let json = store
            .get_node_by_path(1, "/app/core")
            .expect("get_node_by_path should succeed");
        let node: Option<Node> = serde_json::from_str(&json).unwrap();
        let node = node.expect("node at /app/core should exist");
        assert_eq!(node.id, "n2");
    }

    #[test]
    fn get_all_nodes_returns_all() {
        let store = make_test_store();
        let json = store
            .get_all_nodes(1)
            .expect("get_all_nodes should succeed");
        let nodes: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn get_children_returns_direct_children() {
        let store = make_test_store();
        let json = store
            .get_children(1, "n1")
            .expect("get_children should succeed");
        let children: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(children.len(), 2, "n1 should have 2 children (n2, n3)");
        let ids: Vec<&str> = children.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n2"));
        assert!(ids.contains(&"n3"));
    }

    #[test]
    fn get_parent_returns_correct_parent() {
        let store = make_test_store();
        let json = store
            .get_parent(1, "n2")
            .expect("get_parent should succeed");
        let parent: Option<Node> = serde_json::from_str(&json).unwrap();
        let parent = parent.expect("n2 should have a parent");
        assert_eq!(parent.id, "n1");
    }

    #[test]
    fn get_parent_returns_null_for_root() {
        let store = make_test_store();
        let json = store
            .get_parent(1, "n1")
            .expect("get_parent should succeed");
        assert_eq!(json, "null");
    }

    #[test]
    fn get_ancestors_returns_ancestor_chain() {
        let store = make_test_store();
        let json = store
            .get_ancestors(1, "n2")
            .expect("get_ancestors should succeed");
        let ancestors: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(ancestors.len(), 1, "n2 should have 1 ancestor (n1)");
        assert_eq!(ancestors[0].id, "n1");
    }

    #[test]
    fn get_descendants_returns_all_descendants() {
        let store = make_test_store();
        let json = store
            .get_descendants(1, "n1")
            .expect("get_descendants should succeed");
        let descendants: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(descendants.len(), 2, "n1 should have 2 descendants");
    }

    #[test]
    fn get_edges_outgoing_returns_correct_edges() {
        let store = make_test_store();
        let json = store
            .get_edges(1, "n1", "outgoing", None)
            .expect("get_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        assert_eq!(edges.len(), 2, "n1 should have 2 outgoing edges");
    }

    #[test]
    fn get_edges_with_kind_filter() {
        let store = make_test_store();
        let json = store
            .get_edges(1, "n3", "outgoing", Some("depends".to_string()))
            .expect("get_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "n2");
    }

    #[test]
    fn get_edges_incoming() {
        let store = make_test_store();
        let json = store
            .get_edges(1, "n2", "incoming", None)
            .expect("get_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        // n2 is target of: e1 (contains from n1) and e3 (depends from n3)
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn get_edges_both_direction() {
        let store = make_test_store();
        let json = store
            .get_edges(1, "n3", "both", None)
            .expect("get_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        // n3 is: target of e2 (contains from n1), source of e3 (depends on n2)
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn get_all_edges_returns_all() {
        let store = make_test_store();
        let json = store
            .get_all_edges(1, None)
            .expect("get_all_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        assert_eq!(edges.len(), 3);
    }

    #[test]
    fn get_all_edges_with_kind_filter() {
        let store = make_test_store();
        let json = store
            .get_all_edges(1, Some("contains".to_string()))
            .expect("get_all_edges should succeed");
        let edges: Vec<Edge> = serde_json::from_str(&json).unwrap();
        assert_eq!(edges.len(), 2, "should have 2 contains edges");
    }

    #[test]
    fn get_dependencies_direct() {
        let store = make_test_store();
        let json = store
            .get_dependencies(1, "n3", false)
            .expect("get_dependencies should succeed");
        let deps: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(deps.len(), 1, "n3 should depend on n2");
        assert_eq!(deps[0].id, "n2");
    }

    #[test]
    fn get_dependents_direct() {
        let store = make_test_store();
        let json = store
            .get_dependents(1, "n2", false)
            .expect("get_dependents should succeed");
        let dependents: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(dependents.len(), 1, "n2 should have 1 dependent (n3)");
        assert_eq!(dependents[0].id, "n3");
    }

    #[test]
    fn search_by_glob_pattern() {
        let store = make_test_store();
        let json = store.search(1, "/app/*").expect("search should succeed");
        let results: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(results.len(), 2, "should match /app/core and /app/cli");
    }

    #[test]
    fn search_globstar_matches_all() {
        let store = make_test_store();
        let json = store.search(1, "/app/**").expect("search should succeed");
        let results: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(results.len(), 3, "should match /app and its children");
    }

    #[test]
    fn search_exact_path() {
        let store = make_test_store();
        let json = store.search(1, "/app/core").expect("search should succeed");
        let results: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "n2");
    }

    #[test]
    fn invalid_direction_returns_error() {
        let result = parse_direction("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid direction"));
    }

    #[test]
    fn valid_directions_parse_correctly() {
        assert_eq!(parse_direction("outgoing").unwrap(), Direction::Outgoing);
        assert_eq!(parse_direction("incoming").unwrap(), Direction::Incoming);
        assert_eq!(parse_direction("both").unwrap(), Direction::Both);
    }

    #[test]
    fn invalid_edge_kind_returns_error() {
        let result = parse_edge_kind("invalid_kind");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid edge kind"));
    }

    #[test]
    fn valid_edge_kinds_parse_correctly() {
        assert_eq!(parse_edge_kind("contains").unwrap(), EdgeKind::Contains);
        assert_eq!(parse_edge_kind("depends").unwrap(), EdgeKind::Depends);
        assert_eq!(parse_edge_kind("calls").unwrap(), EdgeKind::Calls);
    }
}
