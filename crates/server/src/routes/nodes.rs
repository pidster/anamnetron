//! Node query endpoints.

use axum::extract::{Path, State};
use axum::Json;

use svt_core::model::{Direction, Edge, EdgeKind, Node, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// GET /api/snapshots/{version}/nodes
pub async fn list_nodes(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let nodes = state.store.get_all_nodes(version)?;
    Ok(Json(nodes))
}

/// GET /api/snapshots/{version}/nodes/{id}
pub async fn get_node(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Node>, ApiError> {
    let node = state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    Ok(Json(node))
}

/// GET /api/snapshots/{version}/nodes/{id}/children
pub async fn get_children(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    // Verify node exists
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let children = state.store.get_children(version, &id)?;
    Ok(Json(children))
}

/// GET /api/snapshots/{version}/nodes/{id}/ancestors
pub async fn get_ancestors(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let ancestors = state.store.query_ancestors(version, &id)?;
    Ok(Json(ancestors))
}

/// GET /api/snapshots/{version}/nodes/{id}/dependencies
pub async fn get_dependencies(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges =
        state
            .store
            .get_edges(version, &id, Direction::Outgoing, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

/// GET /api/snapshots/{version}/nodes/{id}/dependents
pub async fn get_dependents(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges =
        state
            .store
            .get_edges(version, &id, Direction::Incoming, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use http_body_util::BodyExt;
    use svt_core::model::{EdgeKind, Node, NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

    use crate::routes::api_router;
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

    fn test_app_with_data() -> Router<()> {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v, &make_node("n1", "/app", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("n2", "/app/core", NodeKind::Service))
            .unwrap();
        store
            .add_edge(
                v,
                &svt_core::model::Edge {
                    id: "e-contains".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Contains,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        api_router(state)
    }

    fn get_request(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn list_nodes_returns_all() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn get_node_returns_single() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n1"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(node["canonical_path"], "/app");
    }

    #[tokio::test]
    async fn get_node_returns_404_for_missing() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/missing"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn get_children_returns_child_nodes() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n1/children"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let children: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["id"], "n2");
    }
}
