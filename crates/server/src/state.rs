//! Application state shared across all route handlers.

use std::sync::Arc;

use svt_core::model::Version;
use svt_core::store::CozoStore;

/// Shared application state.
///
/// Wraps the graph store and tracks which versions are available.
/// CozoStore is internally thread-safe, so no additional synchronization is needed.
#[derive(Debug)]
pub struct AppState {
    /// The graph store backing all queries.
    pub store: CozoStore,
    /// Design snapshot version, if a design was loaded.
    pub design_version: Option<Version>,
    /// Analysis snapshot version, if a project was analyzed.
    pub analysis_version: Option<Version>,
}

/// Type alias for the shared state used in Axum extractors.
pub type SharedState = Arc<AppState>;
