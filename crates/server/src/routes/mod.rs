//! Route modules and router assembly.

pub mod conformance;
pub mod diff;
pub mod edges;
pub mod graph;
pub mod health;
pub mod nodes;
pub mod projects;
pub mod push;
pub mod roots;
pub mod search;
pub mod snapshots;
pub mod store;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::header::CONTENT_SECURITY_POLICY;
use axum::http::HeaderValue;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;

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
        // Project-scoped push (50 MB limit for large analysis snapshots)
        .route("/api/projects/{project}/push", post(push::push_snapshot))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
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
        .route(
            "/api/projects/{project}/snapshots/{version}/roots",
            get(roots::get_project_roots),
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
        // CORS is permissive by default for local development use.
        // For production deployments, configure restrictive origins via environment.
        .layer(CorsLayer::permissive())
        .layer(SetResponseHeaderLayer::if_not_present(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src * data:; font-src 'self' data:",
            ),
        ))
        .with_state(state)
}

/// Build the full router with API routes and optional static file serving.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    use http_body_util::BodyExt;
    use svt_core::model::DEFAULT_PROJECT_ID;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    use crate::state::AppState;

    fn make_state() -> Arc<AppState> {
        let store = CozoStore::new_in_memory().unwrap();
        Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        })
    }

    fn get_request(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn api_router_health_endpoint_responds() {
        let app = api_router(make_state());
        let resp = app.oneshot(get_request("/api/health")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn api_router_unknown_route_returns_404() {
        let app = api_router(make_state());
        let resp = app.oneshot(get_request("/api/nonexistent")).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn full_router_without_static_dir_serves_api() {
        let app = full_router(make_state(), None);
        let resp = app.oneshot(get_request("/api/health")).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn full_router_with_nonexistent_static_dir_serves_api() {
        let fake_dir = std::path::PathBuf::from("/tmp/svt-nonexistent-dir-test");
        let app = full_router(make_state(), Some(fake_dir));
        let resp = app.oneshot(get_request("/api/health")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn full_router_with_existing_static_dir_serves_api_and_fallback() {
        let tmp = std::env::temp_dir().join("svt-static-test");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("index.html"), "<html></html>").unwrap();

        let app = full_router(make_state(), Some(tmp.clone()));

        // API still works
        let resp = app
            .clone()
            .oneshot(get_request("/api/health"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Fallback serves index.html for unknown paths
        let resp = app
            .oneshot(get_request("/some/unknown/path"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(
            String::from_utf8_lossy(&body).contains("<html>"),
            "should serve index.html as fallback"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn api_router_store_info_endpoint_responds() {
        let app = api_router(make_state());
        let resp = app.oneshot(get_request("/api/store/info")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn api_router_projects_endpoint_responds() {
        let app = api_router(make_state());
        let resp = app.oneshot(get_request("/api/projects")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn api_router_snapshots_endpoint_responds() {
        let app = api_router(make_state());
        let resp = app.oneshot(get_request("/api/snapshots")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}
