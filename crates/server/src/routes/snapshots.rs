//! Snapshot listing endpoint.

use axum::extract::{Path, State};
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
    /// Project this snapshot belongs to.
    pub project_id: String,
}

/// GET /api/snapshots (legacy -- defaults to the default project)
pub async fn list_snapshots(
    State(state): State<SharedState>,
) -> Result<Json<Vec<SnapshotResponse>>, ApiError> {
    let project_id = &state.default_project;
    let store = state.read_store()?;
    let snapshots = store.list_snapshots(project_id)?;
    let response: Vec<SnapshotResponse> = snapshots
        .into_iter()
        .map(|s| SnapshotResponse {
            version: s.version,
            kind: s.kind,
            commit_ref: s.commit_ref,
            project_id: s.project_id,
        })
        .collect();
    Ok(Json(response))
}

/// GET /api/projects/{project}/snapshots
pub async fn list_project_snapshots(
    State(state): State<SharedState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<SnapshotResponse>>, ApiError> {
    let store = state.read_store()?;
    if !store.project_exists(&project_id)? {
        return Err(ApiError::NotFound(format!(
            "project '{project_id}' not found"
        )));
    }
    let snapshots = store.list_snapshots(&project_id)?;
    let response: Vec<SnapshotResponse> = snapshots
        .into_iter()
        .map(|s| SnapshotResponse {
            version: s.version,
            kind: s.kind,
            commit_ref: s.commit_ref,
            project_id: s.project_id,
        })
        .collect();
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::model::DEFAULT_PROJECT_ID;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    use crate::state::AppState;

    fn test_app() -> Router {
        // The default project is automatically created by the v1->v2 migration
        let mut store = CozoStore::new_in_memory().unwrap();
        store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        Router::new()
            .route("/api/snapshots", get(list_snapshots))
            .route(
                "/api/projects/{project}/snapshots",
                get(list_project_snapshots),
            )
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
        assert_eq!(json[0]["project_id"], "default");
    }

    #[tokio::test]
    async fn list_project_snapshots_works() {
        let app = test_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/default/snapshots")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.len(), 1);
    }

    #[tokio::test]
    async fn list_project_snapshots_returns_404_for_missing_project() {
        let app = test_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/nonexistent/snapshots")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 404);
    }
}
