//! Root detection endpoint returning topological entry points and terminals.

use axum::extract::{Path, State};
use axum::Json;

use svt_core::model::Version;
use svt_core::roots::{self, RootAnalysis};

use crate::error::ApiError;
use crate::state::SharedState;

/// GET /api/projects/{project}/snapshots/{version}/roots
///
/// Returns topological root analysis for the given snapshot version.
/// The project path segment is validated but the version is globally unique
/// so the store query does not need the project ID.
pub async fn get_project_roots(
    State(state): State<SharedState>,
    Path((_project, version)): Path<(String, Version)>,
) -> Result<Json<RootAnalysis>, ApiError> {
    let store = state.read_store()?;
    let analysis = roots::detect_roots(&*store, version)?;
    Ok(Json(analysis))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use svt_core::model::{
        Edge, EdgeKind, Node, NodeKind, Provenance, SnapshotKind, DEFAULT_PROJECT_ID,
    };
    use svt_core::roots::RootAnalysis;
    use svt_core::store::{CozoStore, GraphStore};
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
    async fn roots_endpoint_returns_empty_for_empty_graph() {
        let state = make_state();
        {
            let mut store = state.store.write().unwrap();
            store
                .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
                .unwrap();
        }

        let app = Router::new()
            .route(
                "/api/projects/{project}/snapshots/{version}/roots",
                get(super::get_project_roots),
            )
            .with_state(state);

        let resp = app
            .oneshot(get_request(&format!(
                "/api/projects/{}/snapshots/1/roots",
                DEFAULT_PROJECT_ID
            )))
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let analysis: RootAnalysis = serde_json::from_slice(&body).unwrap();
        assert!(analysis.call_tree_roots.is_empty());
    }

    #[tokio::test]
    async fn roots_endpoint_detects_call_tree_root() {
        let state = make_state();
        {
            let mut store = state.store.write().unwrap();
            let v = store
                .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
                .unwrap();
            store
                .add_node(
                    v,
                    &Node {
                        id: "main".to_string(),
                        canonical_path: "/app/main".to_string(),
                        qualified_name: None,
                        kind: NodeKind::Unit,
                        sub_kind: "function".to_string(),
                        name: "main".to_string(),
                        language: None,
                        provenance: Provenance::Analysis,
                        source_ref: None,
                        metadata: None,
                    },
                )
                .unwrap();
            store
                .add_node(
                    v,
                    &Node {
                        id: "handler".to_string(),
                        canonical_path: "/app/handler".to_string(),
                        qualified_name: None,
                        kind: NodeKind::Unit,
                        sub_kind: "function".to_string(),
                        name: "handler".to_string(),
                        language: None,
                        provenance: Provenance::Analysis,
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
                        source: "main".to_string(),
                        target: "handler".to_string(),
                        kind: EdgeKind::Calls,
                        provenance: Provenance::Analysis,
                        metadata: None,
                    },
                )
                .unwrap();
        }

        let app = Router::new()
            .route(
                "/api/projects/{project}/snapshots/{version}/roots",
                get(super::get_project_roots),
            )
            .with_state(state);

        let resp = app
            .oneshot(get_request(&format!(
                "/api/projects/{}/snapshots/1/roots",
                DEFAULT_PROJECT_ID
            )))
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let analysis: RootAnalysis = serde_json::from_slice(&body).unwrap();
        assert_eq!(analysis.call_tree_roots.len(), 1);
        assert_eq!(analysis.call_tree_roots[0].node_id, "main");
        assert_eq!(analysis.leaf_sinks.len(), 1);
        assert_eq!(analysis.leaf_sinks[0].node_id, "handler");
    }
}
