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
    let store = state.read_store()?;
    let nodes = store.get_all_nodes(version)?;
    Ok(Json(nodes))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes
pub async fn list_project_nodes(
    State(state): State<SharedState>,
    Path((_project, version)): Path<(String, Version)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let store = state.read_store()?;
    let nodes = store.get_all_nodes(version)?;
    Ok(Json(nodes))
}

/// GET /api/snapshots/{version}/nodes/{id}
pub async fn get_node(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Node>, ApiError> {
    let store = state.read_store()?;
    let node = store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    Ok(Json(node))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes/{id}
pub async fn get_project_node(
    State(state): State<SharedState>,
    Path((_project, version, id)): Path<(String, Version, String)>,
) -> Result<Json<Node>, ApiError> {
    let store = state.read_store()?;
    let node = store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    Ok(Json(node))
}

/// GET /api/snapshots/{version}/nodes/{id}/children
pub async fn get_children(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let children = store.get_children(version, &id)?;
    Ok(Json(children))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes/{id}/children
pub async fn get_project_children(
    State(state): State<SharedState>,
    Path((_project, version, id)): Path<(String, Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let children = store.get_children(version, &id)?;
    Ok(Json(children))
}

/// GET /api/snapshots/{version}/nodes/{id}/ancestors
pub async fn get_ancestors(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let ancestors = store.query_ancestors(version, &id)?;
    Ok(Json(ancestors))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes/{id}/ancestors
pub async fn get_project_ancestors(
    State(state): State<SharedState>,
    Path((_project, version, id)): Path<(String, Version, String)>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let ancestors = store.query_ancestors(version, &id)?;
    Ok(Json(ancestors))
}

/// GET /api/snapshots/{version}/nodes/{id}/dependencies
pub async fn get_dependencies(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = store.get_edges(version, &id, Direction::Outgoing, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes/{id}/dependencies
pub async fn get_project_dependencies(
    State(state): State<SharedState>,
    Path((_project, version, id)): Path<(String, Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = store.get_edges(version, &id, Direction::Outgoing, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

/// GET /api/snapshots/{version}/nodes/{id}/dependents
pub async fn get_dependents(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = store.get_edges(version, &id, Direction::Incoming, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

/// GET /api/projects/{project}/snapshots/{version}/nodes/{id}/dependents
pub async fn get_project_dependents(
    State(state): State<SharedState>,
    Path((_project, version, id)): Path<(String, Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let store = state.read_store()?;
    store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = store.get_edges(version, &id, Direction::Incoming, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use axum::Router;
    use http_body_util::BodyExt;
    use svt_core::model::{EdgeKind, Node, NodeKind, Provenance, SnapshotKind, DEFAULT_PROJECT_ID};
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
        // The default project is automatically created by the v1->v2 migration
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
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
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
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

    #[tokio::test]
    async fn get_children_returns_empty_for_leaf_node() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n2/children"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let children: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(children.is_empty(), "leaf node should have no children");
    }

    #[tokio::test]
    async fn get_children_returns_404_for_missing_node() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/missing/children"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn get_ancestors_returns_parent_chain() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n2/ancestors"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let ancestors: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(
            ancestors.iter().any(|a| a["id"] == "n1"),
            "n2's ancestor should include n1"
        );
    }

    #[tokio::test]
    async fn get_ancestors_returns_empty_for_root_node() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n1/ancestors"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let ancestors: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(ancestors.is_empty(), "root node should have no ancestors");
    }

    #[tokio::test]
    async fn get_ancestors_returns_404_for_missing_node() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/missing/ancestors"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    fn test_app_with_dependencies() -> Router<()> {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store
            .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
            .unwrap();
        store
            .add_node(v, &make_node("n1", "/app", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("n2", "/app/core", NodeKind::Service))
            .unwrap();
        store
            .add_node(v, &make_node("n3", "/app/cli", NodeKind::Service))
            .unwrap();
        // n3 depends on n2
        store
            .add_edge(
                v,
                &svt_core::model::Edge {
                    id: "e-dep".to_string(),
                    source: "n3".to_string(),
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
        api_router(state)
    }

    #[tokio::test]
    async fn get_dependencies_returns_outgoing_depends_edges() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n3/dependencies"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["source"], "n3");
        assert_eq!(edges[0]["target"], "n2");
    }

    #[tokio::test]
    async fn get_dependencies_returns_empty_for_node_without_deps() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n1/dependencies"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(edges.is_empty(), "n1 has no outgoing depends edges");
    }

    #[tokio::test]
    async fn get_dependencies_returns_404_for_missing_node() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/missing/dependencies"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn get_dependents_returns_incoming_depends_edges() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n2/dependents"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["source"], "n3");
        assert_eq!(edges[0]["target"], "n2");
    }

    #[tokio::test]
    async fn get_dependents_returns_empty_for_node_without_dependents() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/n3/dependents"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(edges.is_empty(), "n3 has no incoming depends edges");
    }

    #[tokio::test]
    async fn get_dependents_returns_404_for_missing_node() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request("/api/snapshots/1/nodes/missing/dependents"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    // --- Project-scoped endpoint tests ---

    #[tokio::test]
    async fn list_project_nodes_returns_all() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/projects/default/snapshots/1/nodes"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn get_project_node_returns_single() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request("/api/projects/default/snapshots/1/nodes/n1"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(node["canonical_path"], "/app");
    }

    #[tokio::test]
    async fn get_project_node_returns_404_for_missing() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request(
                "/api/projects/default/snapshots/1/nodes/missing",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn get_project_children_returns_child_nodes() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request(
                "/api/projects/default/snapshots/1/nodes/n1/children",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let children: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["id"], "n2");
    }

    #[tokio::test]
    async fn get_project_ancestors_returns_parent_chain() {
        let app = test_app_with_data();
        let resp = app
            .oneshot(get_request(
                "/api/projects/default/snapshots/1/nodes/n2/ancestors",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let ancestors: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(
            ancestors.iter().any(|a| a["id"] == "n1"),
            "n2's ancestor should include n1"
        );
    }

    #[tokio::test]
    async fn get_project_dependencies_returns_edges() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request(
                "/api/projects/default/snapshots/1/nodes/n3/dependencies",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["target"], "n2");
    }

    #[tokio::test]
    async fn get_project_dependents_returns_edges() {
        let app = test_app_with_dependencies();
        let resp = app
            .oneshot(get_request(
                "/api/projects/default/snapshots/1/nodes/n2/dependents",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["source"], "n3");
    }
}
