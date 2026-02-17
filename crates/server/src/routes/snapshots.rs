//! Snapshot listing endpoint.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use svt_core::model::SnapshotKind;
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Snapshot summary for API response.
#[derive(Serialize)]
pub struct SnapshotResponse {
    /// Snapshot version number.
    pub version: u64,
    /// Snapshot kind (design, analysis, import).
    pub kind: SnapshotKind,
    /// Git commit ref, if available.
    pub commit_ref: Option<String>,
}

/// GET /api/snapshots
pub async fn list_snapshots(
    State(state): State<SharedState>,
) -> Result<Json<Vec<SnapshotResponse>>, ApiError> {
    let snapshots = state.store.list_snapshots()?;
    let response: Vec<SnapshotResponse> = snapshots
        .into_iter()
        .map(|s| SnapshotResponse {
            version: s.version,
            kind: s.kind,
            commit_ref: s.commit_ref,
        })
        .collect();
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    use crate::state::AppState;

    fn test_app() -> Router {
        let mut store = CozoStore::new_in_memory().unwrap();
        store.create_snapshot(SnapshotKind::Design, None).unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: Some(1),
            analysis_version: None,
        });
        Router::new()
            .route("/api/snapshots", get(list_snapshots))
            .with_state(state)
    }

    #[tokio::test]
    async fn list_snapshots_returns_loaded_versions() {
        let app = test_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/snapshots")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["version"], 1);
        assert_eq!(json[0]["kind"], "design");
    }
}
