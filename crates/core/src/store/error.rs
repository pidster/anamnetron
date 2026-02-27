//! Error types for graph store operations.

use crate::model::{NodeId, ProjectId, Version};

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

    /// Schema version mismatch (store is newer than this binary).
    #[error("schema version mismatch: store has version {found}, but this binary supports up to version {expected}")]
    SchemaMismatch {
        /// The maximum schema version this binary supports.
        expected: u32,
        /// The schema version found in the store.
        found: u32,
    },

    /// The store is corrupt or contains invalid data.
    #[error("corrupt store: {0}")]
    CorruptStore(String),

    /// An internal store error.
    #[error("store error: {0}")]
    Internal(String),

    /// A project was not found.
    #[error("project not found: {0}")]
    ProjectNotFound(ProjectId),

    /// Attempted to create a project with a duplicate ID.
    #[error("duplicate project: {0}")]
    DuplicateProject(ProjectId),

    /// An invalid project ID was provided.
    #[error("invalid project ID: {0}")]
    InvalidProjectId(String),
}
