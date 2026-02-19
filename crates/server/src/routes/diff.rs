//! Snapshot diff endpoint.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use svt_core::diff::{self, SnapshotDiff};
use svt_core::model::Version;

use crate::error::ApiError;
use crate::state::SharedState;

/// Query parameters for diff endpoint.
#[derive(Deserialize)]
pub struct DiffParams {
    /// Base snapshot version.
    pub from: Version,
    /// Target snapshot version.
    pub to: Version,
}

/// GET /api/diff?from=V1&to=V2
pub async fn get_diff(
    State(state): State<SharedState>,
    Query(params): Query<DiffParams>,
) -> Result<Json<SnapshotDiff>, ApiError> {
    let result = diff::diff_snapshots(&state.store, params.from, params.to)?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::model::{Node, NodeKind, Provenance, SnapshotKind};
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
            .route("/api/diff", get(get_diff))
            .with_state(state)
    }

    #[tokio::test]
    async fn diff_identical_snapshots_shows_no_changes() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v1, &make_node("n1", "/app", NodeKind::System))
            .unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v2, &make_node("n2", "/app", NodeKind::System))
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v1),
            analysis_version: None,
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/diff?from={v1}&to={v2}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let diff: SnapshotDiff = serde_json::from_slice(&body).unwrap();
        assert_eq!(diff.from_version, v1);
        assert_eq!(diff.to_version, v2);
        assert!(diff.node_changes.is_empty(), "same nodes = no changes");
    }

    #[tokio::test]
    async fn diff_added_node_detected() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v1, &make_node("n1", "/app", NodeKind::System))
            .unwrap();

        let v2 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v2, &make_node("n2", "/app", NodeKind::System))
            .unwrap();
        store
            .add_node(v2, &make_node("n3", "/app/svc", NodeKind::Service))
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v1),
            analysis_version: None,
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/diff?from={v1}&to={v2}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let diff: SnapshotDiff = serde_json::from_slice(&body).unwrap();
        assert_eq!(diff.summary.nodes_added, 1);
        assert_eq!(diff.node_changes.len(), 1);
    }
}
