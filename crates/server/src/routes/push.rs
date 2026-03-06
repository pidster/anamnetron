//! Snapshot push endpoint for bulk data ingestion.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use svt_core::model::{Constraint, Edge, Node, Project, SnapshotKind};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Request body for pushing a snapshot.
#[derive(Debug, Deserialize)]
pub struct PushRequest {
    /// Snapshot kind: "design" or "analysis".
    pub kind: SnapshotKind,
    /// Optional git commit reference.
    pub commit_ref: Option<String>,
    /// Nodes to insert.
    pub nodes: Vec<Node>,
    /// Edges to insert.
    pub edges: Vec<Edge>,
    /// Optional constraints to insert.
    pub constraints: Option<Vec<Constraint>>,
}

/// Response body for a push operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    /// The version number of the created snapshot.
    pub version: u64,
    /// Project the snapshot was created in.
    pub project_id: String,
    /// Number of nodes inserted.
    pub nodes_created: usize,
    /// Number of edges inserted.
    pub edges_created: usize,
    /// Number of constraints inserted.
    pub constraints_created: usize,
}

/// POST /api/projects/{project}/push
///
/// Push a complete snapshot (nodes, edges, constraints) into a project.
/// Auto-creates the project if it does not exist.
pub async fn push_snapshot(
    State(state): State<SharedState>,
    Path(project_id): Path<String>,
    Json(req): Json<PushRequest>,
) -> Result<(axum::http::StatusCode, Json<PushResponse>), ApiError> {
    svt_core::model::validate_project_id(&project_id).map_err(ApiError::BadRequest)?;

    let mut store = state.write_store()?;

    // Auto-create project if it doesn't exist
    if !store.project_exists(&project_id)? {
        let now = chrono::Utc::now().to_rfc3339();
        store.create_project(&Project {
            id: project_id.clone(),
            name: project_id.clone(),
            created_at: now,
            description: None,
            metadata: None,
        })?;
    }

    let commit_ref = req.commit_ref.as_deref();
    let version = store.create_snapshot(&project_id, req.kind, commit_ref)?;

    let nodes_count = req.nodes.len();
    let edges_count = req.edges.len();

    if !req.nodes.is_empty() {
        store.add_nodes_batch(version, &req.nodes)?;
    }
    if !req.edges.is_empty() {
        store.add_edges_batch(version, &req.edges)?;
    }

    let constraints_count = if let Some(constraints) = &req.constraints {
        for c in constraints {
            store.add_constraint(version, c)?;
        }
        constraints.len()
    } else {
        0
    };

    let response = PushResponse {
        version,
        project_id,
        nodes_created: nodes_count,
        edges_created: edges_count,
        constraints_created: constraints_count,
    };

    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    use axum::routing::post;
    use axum::Router;
    use http_body_util::BodyExt;
    use svt_core::model::DEFAULT_PROJECT_ID;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    use crate::state::AppState;

    fn test_app() -> Router {
        let store = CozoStore::new_in_memory().unwrap();
        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        Router::new()
            .route("/api/projects/{project}/push", post(push_snapshot))
            .with_state(state)
    }

    #[tokio::test]
    async fn push_creates_snapshot_with_nodes_and_edges() {
        let app = test_app();
        let body = serde_json::json!({
            "kind": "analysis",
            "commit_ref": "abc123",
            "nodes": [
                {
                    "id": "n1",
                    "canonical_path": "/app/a",
                    "kind": "component",
                    "sub_kind": "module",
                    "name": "a",
                    "provenance": "analysis"
                },
                {
                    "id": "n2",
                    "canonical_path": "/app/b",
                    "kind": "component",
                    "sub_kind": "module",
                    "name": "b",
                    "provenance": "analysis"
                }
            ],
            "edges": [
                {
                    "id": "e1",
                    "source": "n1",
                    "target": "n2",
                    "kind": "depends",
                    "provenance": "analysis"
                }
            ]
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects/my-project/push")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 201);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let result: PushResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.project_id, "my-project");
        assert_eq!(result.nodes_created, 2);
        assert_eq!(result.edges_created, 1);
        assert_eq!(result.constraints_created, 0);
        assert!(result.version > 0);
    }

    #[tokio::test]
    async fn push_with_constraints_creates_all() {
        let app = test_app();
        let body = serde_json::json!({
            "kind": "design",
            "nodes": [
                {
                    "id": "n1",
                    "canonical_path": "/app/a",
                    "kind": "component",
                    "sub_kind": "module",
                    "name": "a",
                    "provenance": "design"
                }
            ],
            "edges": [],
            "constraints": [
                {
                    "id": "c1",
                    "name": "no-cycles",
                    "kind": "no_circular_dependency",
                    "scope": "/app/**",
                    "message": "No cycles allowed",
                    "severity": "error"
                }
            ]
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects/my-project/push")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 201);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let result: PushResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.nodes_created, 1);
        assert_eq!(result.edges_created, 0);
        assert_eq!(result.constraints_created, 1);
    }

    #[tokio::test]
    async fn push_empty_snapshot_succeeds() {
        let app = test_app();
        let body = serde_json::json!({
            "kind": "design",
            "nodes": [],
            "edges": []
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects/empty-proj/push")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 201);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let result: PushResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.nodes_created, 0);
        assert_eq!(result.edges_created, 0);
        assert_eq!(result.constraints_created, 0);
    }

    #[tokio::test]
    async fn push_rejects_invalid_project_id() {
        let app = test_app();
        let body = serde_json::json!({
            "kind": "design",
            "nodes": [],
            "edges": []
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects/INVALID/push")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 400);
    }
}
