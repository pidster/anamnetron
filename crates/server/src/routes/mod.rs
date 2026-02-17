//! Route modules and router assembly.

pub mod edges;
pub mod health;
pub mod nodes;
pub mod snapshots;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::state::AppState;

/// Build the full API router with all routes and middleware.
pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/snapshots", get(snapshots::list_snapshots))
        .route("/api/snapshots/{version}/nodes", get(nodes::list_nodes))
        .route("/api/snapshots/{version}/nodes/{id}", get(nodes::get_node))
        .route(
            "/api/snapshots/{version}/nodes/{id}/children",
            get(nodes::get_children),
        )
        .route(
            "/api/snapshots/{version}/nodes/{id}/ancestors",
            get(nodes::get_ancestors),
        )
        .route(
            "/api/snapshots/{version}/nodes/{id}/dependencies",
            get(nodes::get_dependencies),
        )
        .route(
            "/api/snapshots/{version}/nodes/{id}/dependents",
            get(nodes::get_dependents),
        )
        .route("/api/snapshots/{version}/edges", get(edges::list_edges))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
