//! Node search endpoint.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use svt_core::canonical::canonical_path_matches;
use svt_core::model::{Node, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Query parameters for search.
#[derive(Deserialize)]
pub struct SearchParams {
    /// Glob pattern to match canonical paths (e.g., "/svt/core/**").
    pub path: String,
    /// Version to search in.
    pub version: Version,
}

/// GET /api/search?path=GLOB&version=V
pub async fn search_nodes(
    State(state): State<SharedState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<Node>>, ApiError> {
    if params.path.is_empty() {
        return Err(ApiError::BadRequest(
            "path parameter is required".to_string(),
        ));
    }
    let all_nodes = state.store.get_all_nodes(params.version)?;
    let matched: Vec<Node> = all_nodes
        .into_iter()
        .filter(|n| canonical_path_matches(&n.canonical_path, &params.path))
        .collect();
    Ok(Json(matched))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::model::{NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

    use crate::state::AppState;

    fn make_node(id: &str, path: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            canonical_path: path.to_string(),
            qualified_name: None,
            kind,
            sub_kind: "test".to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        }
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/api/search", get(search_nodes))
            .with_state(state)
    }

    #[tokio::test]
    async fn search_by_glob_pattern() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v, &make_node("n1", "/app", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("n2", "/app/core", NodeKind::Service))
            .unwrap();
        store
            .add_node(v, &make_node("n3", "/app/cli", NodeKind::Service))
            .unwrap();
        store
            .add_node(v, &make_node("n4", "/other", NodeKind::System))
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/search?path=/app/**&version=1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        // /app/core and /app/cli match /app/**, plus /app itself (globstar matches base)
        assert!(
            nodes.len() >= 2,
            "should match at least /app/core and /app/cli, got {}",
            nodes.len()
        );
    }

    #[tokio::test]
    async fn search_empty_path_returns_400() {
        let store = CozoStore::new_in_memory().unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: None,
            analysis_version: None,
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/search?path=&version=1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }
}
