//! Application state shared across all route handlers.

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use svt_core::store::CozoStore;

use crate::error::ApiError;

/// Shared application state.
///
/// Wraps the graph store in an `RwLock` so that read-only handlers can share
/// concurrent access while write handlers (push, create project) can obtain
/// exclusive access. CozoStore's internal `DbInstance` is already thread-safe,
/// so the lock overhead is minimal.
#[derive(Debug)]
pub struct AppState {
    /// The graph store backing all queries, wrapped for interior mutability.
    pub store: RwLock<CozoStore>,
    /// Default project ID (specified on the command line or "default").
    pub default_project: String,
}

impl AppState {
    /// Acquire a read lock on the store.
    pub fn read_store(&self) -> Result<RwLockReadGuard<'_, CozoStore>, ApiError> {
        self.store
            .read()
            .map_err(|e| ApiError::Internal(format!("store lock poisoned: {e}")))
    }

    /// Acquire a write lock on the store.
    pub fn write_store(&self) -> Result<RwLockWriteGuard<'_, CozoStore>, ApiError> {
        self.store
            .write()
            .map_err(|e| ApiError::Internal(format!("store lock poisoned: {e}")))
    }
}

/// Type alias for the shared state used in Axum extractors.
pub type SharedState = Arc<AppState>;
