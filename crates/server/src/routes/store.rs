//! Store info endpoint.

use axum::extract::State;
use axum::Json;

use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// GET /api/store/info
///
/// Returns store metadata: schema version, snapshot count, and per-snapshot
/// node/edge counts.
pub async fn store_info(
    State(state): State<SharedState>,
) -> Result<Json<svt_core::store::StoreInfo>, ApiError> {
    let info = state.store.store_info()?;
    Ok(Json(info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::model::SnapshotKind;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    use crate::state::AppState;

    fn test_app_empty() -> Router {
        let store = CozoStore::new_in_memory().unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: None,
            analysis_version: None,
        });
        Router::new()
            .route("/api/store/info", get(store_info))
            .with_state(state)
    }

    fn test_app_with_snapshot() -> Router {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n1".to_string(),
                    canonical_path: "/svc/a".to_string(),
                    qualified_name: None,
                    kind: svt_core::model::NodeKind::Component,
                    sub_kind: "module".to_string(),
                    name: "a".to_string(),
                    language: None,
                    provenance: svt_core::model::Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        Router::new()
            .route("/api/store/info", get(store_info))
            .with_state(state)
    }

    #[tokio::test]
    async fn store_info_endpoint_returns_schema_version() {
        let app = test_app_empty();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/store/info")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["snapshot_count"], 0);
    }

    #[tokio::test]
    async fn store_info_endpoint_shows_snapshot_details() {
        let app = test_app_with_snapshot();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/store/info")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["snapshot_count"], 1);

        let snapshots = json["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["version"], 1);
        assert_eq!(snapshots[0]["kind"], "design");
        assert_eq!(snapshots[0]["node_count"], 1);
        assert_eq!(snapshots[0]["edge_count"], 0);
    }
}
