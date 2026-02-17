//! Conformance evaluation endpoints.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use svt_core::conformance::{self, ConformanceReport};
use svt_core::model::Version;

use crate::error::ApiError;
use crate::state::SharedState;

/// Query parameters for full conformance comparison.
#[derive(Deserialize)]
pub struct ConformanceParams {
    /// Design version.
    pub design: Version,
    /// Analysis version.
    pub analysis: Version,
}

/// GET /api/conformance/design/{version}
pub async fn evaluate_design(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
) -> Result<Json<ConformanceReport>, ApiError> {
    let report = conformance::evaluate_design(&state.store, version)?;
    Ok(Json(report))
}

/// GET /api/conformance?design=V&analysis=V
pub async fn evaluate_conformance(
    State(state): State<SharedState>,
    Query(params): Query<ConformanceParams>,
) -> Result<Json<ConformanceReport>, ApiError> {
    let report = conformance::evaluate(&state.store, params.design, params.analysis)?;
    Ok(Json(report))
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
            .route("/api/conformance/design/{version}", get(evaluate_design))
            .route("/api/conformance", get(evaluate_conformance))
            .with_state(state)
    }

    #[tokio::test]
    async fn design_conformance_returns_report() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v, &make_node("n1", "/app", NodeKind::System))
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
                    .uri("/api/conformance/design/1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let report: ConformanceReport = serde_json::from_slice(&body).unwrap();
        assert_eq!(report.design_version, 1);
        assert!(report.summary.passed >= 2, "structural checks should pass");
    }

    #[tokio::test]
    async fn full_conformance_returns_report() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System))
            .unwrap();

        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System))
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(dv),
            analysis_version: Some(av),
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/conformance?design={dv}&analysis={av}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let report: ConformanceReport = serde_json::from_slice(&body).unwrap();
        assert_eq!(report.design_version, dv);
        assert_eq!(report.analysis_version, Some(av));
    }
}
