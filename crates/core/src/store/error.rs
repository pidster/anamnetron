//! Error types for graph store operations.

use crate::model::{NodeId, Version};

/// Errors that can occur during graph store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A node was not found.
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    /// A version was not found.
    #[error("version not found: {0}")]
    VersionNotFound(Version),

    /// Attempted to add a node with a duplicate ID.
    #[error("duplicate node: {0}")]
    DuplicateNode(String),

    /// Attempted to add an edge with a duplicate ID.
    #[error("duplicate edge: {0}")]
    DuplicateEdge(String),

    /// An edge references a node that does not exist.
    #[error("invalid reference: edge {edge_id} references unknown node {node_id}")]
    InvalidReference {
        /// The edge with the invalid reference.
        edge_id: String,
        /// The node ID that was not found.
        node_id: String,
    },

    /// An internal store error.
    #[error("store error: {0}")]
    Internal(String),
}
