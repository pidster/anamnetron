//! Graph store trait and backend implementations.

mod error;

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
}
