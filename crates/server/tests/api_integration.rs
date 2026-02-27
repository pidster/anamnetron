//! Integration tests for the svt-server API.
//!
//! Since svt-server is a binary crate, we cannot import its internal modules.
//! Instead, we build a minimal test router using svt-core APIs directly,
//! replicating the server's behaviour for the endpoints under test.
//! Data comes from the project's own `design/architecture.yaml`.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::Request;
use axum::routing::get;
use axum::Json;
use http_body_util::BodyExt;
use tower::ServiceExt;

use svt_core::conformance::{self, ConstraintRegistry};
use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::model::{Version, DEFAULT_PROJECT_ID};
use svt_core::store::{CozoStore, GraphStore};

/// Shared test state: the store plus the design version.
type TestState = Arc<(CozoStore, Version)>;

// -- Inline handler functions that mirror the real server routes --

async fn list_snapshots(State(state): State<TestState>) -> Json<serde_json::Value> {
    let snapshots = state.0.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    Json(serde_json::to_value(snapshots).unwrap())
}

async fn list_nodes(
    State(state): State<TestState>,
    Path(version): Path<Version>,
) -> Json<serde_json::Value> {
    let nodes = state.0.get_all_nodes(version).unwrap();
    Json(serde_json::to_value(nodes).unwrap())
}

async fn list_edges(
    State(state): State<TestState>,
    Path(version): Path<Version>,
) -> Json<serde_json::Value> {
    let edges = state.0.get_all_edges(version, None).unwrap();
    Json(serde_json::to_value(edges).unwrap())
}

async fn design_conformance(
    State(state): State<TestState>,
    Path(version): Path<Version>,
) -> Json<serde_json::Value> {
    let registry = ConstraintRegistry::with_defaults();
    let report = conformance::evaluate_design(&state.0, version, &registry).unwrap();
    Json(serde_json::to_value(report).unwrap())
}

/// Build a test router loaded with the project's own architecture design.
fn test_router_with_design() -> axum::Router {
    let design_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("design/architecture.yaml");

    let content = std::fs::read_to_string(&design_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", design_path.display()));
    let doc = interchange::parse_yaml(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", design_path.display()));

    // The default project is automatically created by the v1->v2 migration
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, DEFAULT_PROJECT_ID, &doc).unwrap();

    let state: TestState = Arc::new((store, version));

    axum::Router::new()
        .route("/api/snapshots", get(list_snapshots))
        .route("/api/snapshots/{version}/nodes", get(list_nodes))
        .route("/api/snapshots/{version}/edges", get(list_edges))
        .route("/api/conformance/design/{version}", get(design_conformance))
        .with_state(state)
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn snapshots_endpoint_returns_design() {
    let app = test_router_with_design();
    let resp = app.oneshot(get_request("/api/snapshots")).await.unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let snapshots: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!snapshots.is_empty(), "should have at least one snapshot");
    assert_eq!(
        snapshots[0]["kind"], "design",
        "first snapshot should be a design snapshot"
    );
}

#[tokio::test]
async fn nodes_endpoint_returns_architecture_nodes() {
    let app = test_router_with_design();
    let resp = app
        .oneshot(get_request("/api/snapshots/1/nodes"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(
        nodes.len() > 10,
        "architecture.yaml should produce more than 10 nodes, got {}",
        nodes.len()
    );

    // Verify the root /svt node exists
    let has_svt = nodes
        .iter()
        .any(|n| n["canonical_path"].as_str() == Some("/svt"));
    assert!(has_svt, "should contain the root /svt node");
}

#[tokio::test]
async fn edges_endpoint_returns_architecture_edges() {
    let app = test_router_with_design();
    let resp = app
        .oneshot(get_request("/api/snapshots/1/edges"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let edges: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(
        !edges.is_empty(),
        "architecture.yaml defines dependency and containment edges"
    );
}

#[tokio::test]
async fn conformance_endpoint_returns_report() {
    let app = test_router_with_design();
    let resp = app
        .oneshot(get_request("/api/conformance/design/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let report: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let passed = report["summary"]["passed"]
        .as_u64()
        .expect("summary.passed should be a number");
    assert!(
        passed >= 2,
        "design-only conformance should pass at least 2 structural checks, got {passed}"
    );
}
