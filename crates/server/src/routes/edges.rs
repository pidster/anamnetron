//! Edge query endpoint.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use svt_core::model::{Edge, EdgeKind, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Query parameters for edge filtering.
#[derive(Deserialize)]
pub struct EdgeFilter {
    /// Filter by edge kind (e.g., "depends", "contains").
    pub kind: Option<EdgeKind>,
}

/// GET /api/snapshots/{version}/edges
pub async fn list_edges(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
    Query(filter): Query<EdgeFilter>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    let edges = store.get_all_edges(version, filter.kind)?;
    Ok(Json(edges))
}

/// GET /api/projects/{project}/snapshots/{version}/edges
pub async fn list_project_edges(
    State(state): State<SharedState>,
    Path((_project, version)): Path<(String, Version)>,
    Query(filter): Query<EdgeFilter>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    let edges = store.get_all_edges(version, filter.kind)?;
    Ok(Json(edges))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use http_body_util::BodyExt;
    use svt_core::model::{Edge, EdgeKind, NodeKind, Provenance, SnapshotKind, DEFAULT_PROJECT_ID};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

    use crate::routes::api_router;
    use crate::state::AppState;

    /// The default project is automatically created by the v1->v2 migration.
    fn make_store_with_project() -> CozoStore {
        CozoStore::new_in_memory().unwrap()
    }

    #[tokio::test]
    async fn list_edges_returns_all() {
        let mut store = make_store_with_project();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n1".to_string(),
                    canonical_path: "/a".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "a".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n2".to_string(),
                    canonical_path: "/b".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "b".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_edge(
                v,
                &Edge {
                    id: "e1".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Depends,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();

        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        let app = api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/snapshots/1/edges")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["kind"], "depends");
    }

    #[tokio::test]
    async fn list_project_edges_returns_all() {
        let mut store = make_store_with_project();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n1".to_string(),
                    canonical_path: "/a".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "a".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n2".to_string(),
                    canonical_path: "/b".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "b".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_edge(
                v,
                &Edge {
                    id: "e1".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Depends,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();

        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        let app = crate::routes::api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/default/snapshots/1/edges")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["kind"], "depends");
    }

    #[tokio::test]
    async fn list_edges_with_kind_filter() {
        let mut store = make_store_with_project();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n1".to_string(),
                    canonical_path: "/a".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "a".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_node(
                v,
                &svt_core::model::Node {
                    id: "n2".to_string(),
                    canonical_path: "/b".to_string(),
                    qualified_name: None,
                    kind: NodeKind::System,
                    sub_kind: "test".to_string(),
                    name: "b".to_string(),
                    language: None,
                    provenance: Provenance::Design,
                    source_ref: None,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_edge(
                v,
                &Edge {
                    id: "e1".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Depends,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();
        store
            .add_edge(
                v,
                &Edge {
                    id: "e2".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Contains,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();

        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        let app = api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/snapshots/1/edges?kind=depends")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1, "should filter to only depends edges");
    }
}
