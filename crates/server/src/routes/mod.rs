//! Route modules and router assembly.

pub mod conformance;
pub mod diff;
pub mod edges;
pub mod graph;
pub mod health;
pub mod nodes;
pub mod projects;
pub mod push;
pub mod search;
pub mod snapshots;
pub mod store;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::state::AppState;

/// Build the full API router with all routes and middleware.
pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health
        .route("/api/health", get(health::health))
        // Project CRUD
        .route(
            "/api/projects",
            get(projects::list_projects).post(projects::create_project_handler),
        )
        .route(
            "/api/projects/{project}",
            get(projects::get_project_handler),
        )
        // Project-scoped push
        .route("/api/projects/{project}/push", post(push::push_snapshot))
        // Project-scoped snapshots
        .route(
            "/api/projects/{project}/snapshots",
            get(snapshots::list_project_snapshots),
        )
        // Project-scoped snapshot detail routes
        .route(
            "/api/projects/{project}/snapshots/{version}/graph",
            get(graph::get_project_graph),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes",
            get(nodes::list_project_nodes),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes/{id}",
            get(nodes::get_project_node),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes/{id}/children",
            get(nodes::get_project_children),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes/{id}/ancestors",
            get(nodes::get_project_ancestors),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes/{id}/dependencies",
            get(nodes::get_project_dependencies),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/nodes/{id}/dependents",
            get(nodes::get_project_dependents),
        )
        .route(
            "/api/projects/{project}/snapshots/{version}/edges",
            get(edges::list_project_edges),
        )
        // Legacy routes (default project)
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
        .route("/api/snapshots/{version}/graph", get(graph::get_graph))
        .route(
            "/api/conformance/design/{version}",
            get(conformance::evaluate_design),
        )
        .route("/api/conformance", get(conformance::evaluate_conformance))
        .route("/api/diff", get(diff::get_diff))
        .route("/api/search", get(search::search_nodes))
        .route("/api/store/info", get(store::store_info))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Build the full router with API routes and optional static file serving.
///
/// If `static_dir` is provided and the directory exists, serves static files at `/`.
/// API routes take priority over static files.
pub fn full_router(state: Arc<AppState>, static_dir: Option<std::path::PathBuf>) -> Router {
    let router = api_router(state);

    if let Some(dir) = static_dir {
        if dir.exists() {
            tracing::info!(path = %dir.display(), "serving static files");
            return router.fallback_service(
                tower_http::services::ServeDir::new(&dir)
                    .fallback(tower_http::services::ServeFile::new(dir.join("index.html"))),
            );
        }
    }

    router
}
