//! Graph store trait and backend implementations.

pub mod cozo;
mod error;

pub use cozo::CozoStore;
pub use error::StoreError;

use crate::model::*;

/// Result type for graph store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Abstract interface for the graph store.
///
/// All operations require an explicit version -- there is no implicit
/// "current version". This keeps the store stateless and makes
/// conformance (comparing two versions) natural.
pub trait GraphStore {
    /// Create a new snapshot and return its version number.
    fn create_snapshot(&mut self, kind: SnapshotKind, commit_ref: Option<&str>) -> Result<Version>;

    /// List all snapshots in version order.
    fn list_snapshots(&self) -> Result<Vec<Snapshot>>;

    /// Get the latest version for a given snapshot kind, or None if no snapshots exist.
    fn latest_version(&self, kind: SnapshotKind) -> Result<Option<Version>>;

    /// Add a single node to the store.
    fn add_node(&mut self, version: Version, node: &Node) -> Result<()>;

    /// Add multiple nodes in a single batch operation.
    fn add_nodes_batch(&mut self, version: Version, nodes: &[Node]) -> Result<()>;

    /// Get a node by its ID within a version.
    fn get_node(&self, version: Version, id: &NodeId) -> Result<Option<Node>>;

    /// Get a node by its canonical path within a version.
    fn get_node_by_path(&self, version: Version, canonical_path: &str) -> Result<Option<Node>>;

    /// Add a single edge to the store.
    fn add_edge(&mut self, version: Version, edge: &Edge) -> Result<()>;

    /// Add multiple edges in a single batch operation.
    fn add_edges_batch(&mut self, version: Version, edges: &[Edge]) -> Result<()>;

    /// Get edges connected to a node, filtered by direction and optionally by kind.
    fn get_edges(
        &self,
        version: Version,
        node_id: &NodeId,
        direction: Direction,
        kind: Option<EdgeKind>,
    ) -> Result<Vec<Edge>>;

    /// Get the direct children of a node (via `Contains` edges where the node is the source).
    fn get_children(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>>;

    /// Get the parent of a node (via `Contains` edge where the node is the target).
    fn get_parent(&self, version: Version, node_id: &NodeId) -> Result<Option<Node>>;

    /// Get all ancestors of a node, from parent up to root. Ordered from immediate parent to root.
    fn query_ancestors(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>>;

    /// Get all descendants of a node, optionally filtered by a [`NodeFilter`].
    fn query_descendants(
        &self,
        version: Version,
        node_id: &NodeId,
        filter: Option<&NodeFilter>,
    ) -> Result<Vec<Node>>;

    /// Get dependencies of a node (nodes it depends on via `Depends` edges).
    ///
    /// If `transitive` is true, follows the dependency chain recursively.
    fn query_dependencies(
        &self,
        version: Version,
        node_id: &NodeId,
        transitive: bool,
    ) -> Result<Vec<Node>>;

    /// Get dependents of a node (nodes that depend on it via `Depends` edges).
    ///
    /// If `transitive` is true, follows the dependent chain recursively.
    fn query_dependents(
        &self,
        version: Version,
        node_id: &NodeId,
        transitive: bool,
    ) -> Result<Vec<Node>>;
}
