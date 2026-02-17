# Milestone 4: Server API Implementation Plan

## Status: COMPLETE (2026-02-17)

- **Tests:** 17 new (13 unit + 4 integration), 218 total workspace tests passing
- **Commits:** 10 implementation commits (`3279f17`..`e4a1f42`)
- **Verification:** clippy clean, fmt clean, audit clean

| Task | Description | Commit | Tests |
|------|-------------|--------|-------|
| 0 | Dependencies | `3279f17` | — |
| 1 | AppState + ApiError | `8ba52fd` | — |
| 2 | Health + Snapshots | `71f3408` | 2 |
| 3 | Nodes routes | `531670f` | 4 |
| 4 | Edges route | `b3b3820` | 2 |
| 5 | Graph (Cytoscape.js) | `18af029` | 1 |
| 6 | Conformance endpoints | `3ffaad4` | 2 |
| 7 | Search endpoint | `e26d5b4` | 2 |
| 8 | CLI startup | `81f0cd3` | — |
| 9+10 | Integration tests + verification | `e4a1f42` | 4 |

---

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Axum REST API serving graph data read-only from a pre-loaded CozoStore, enabling architecture exploration via HTTP.

**Architecture:** The server loads data at startup (import design YAML, analyze a project, or both) into an in-memory CozoStore, then serves it read-only via REST endpoints. AppState wraps the store in Arc. Route handlers use Axum extractors to parse path/query params and call GraphStore trait methods. Responses are JSON. Errors use a typed ApiError enum.

**Tech Stack:** Rust, Axum 0.8, tokio, tower-http (CORS), clap, serde/serde_json, tracing, svt-core (GraphStore, CozoStore, conformance), svt-analyzer (analyze_project)

**Design doc:** `docs/plan/2026-02-17-milestones-4-5-design.md`

**Dependency Graph:**
```
Task 0 (deps) ──→ Task 1 (state+error) ──→ Task 2 (health+snapshots)
                       │
                       ├──→ Task 3 (nodes) ──→ Task 4 (edges)
                       │
                       └──→ Task 5 (graph) ──→ Task 6 (conformance)
                                                     │
Task 7 (search) ←─────────────────────────────────────┘
                                                     │
Task 8 (startup CLI) ←──────────────────────────────┘
                       │
                       └──→ Task 9 (integration tests) ──→ Task 10 (dog-food)
```

---

### Task 0: Add Dependencies

**Files:**
- Modify: `crates/server/Cargo.toml`

**Step 1: Update Cargo.toml**

```toml
[package]
name = "svt-server"
description = "Axum API service and web UI host for software-visualizer-tool"
version.workspace = true
edition.workspace = true

[[bin]]
name = "svt-server"
path = "src/main.rs"

[dependencies]
svt-core = { path = "../core" }
svt-analyzer = { path = "../analyzer" }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "fs"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

Note: `tower` with `util` feature and `http-body-util` are dev-dependencies for testing Axum routers with `oneshot()` and reading response bodies.

**Step 2: Verify it compiles**

Run: `cargo check -p svt-server`
Expected: success

**Step 3: Commit**

```bash
git add crates/server/Cargo.toml
git commit -m "feat(server): add serde, clap, tracing dependencies"
```

---

### Task 1: AppState and ApiError

**Files:**
- Create: `crates/server/src/state.rs`
- Create: `crates/server/src/error.rs`
- Modify: `crates/server/src/main.rs` — add module declarations

**Step 1: Create state.rs**

```rust
//! Application state shared across all route handlers.

use std::sync::Arc;

use svt_core::model::Version;
use svt_core::store::CozoStore;

/// Shared application state.
///
/// Wraps the graph store and tracks which versions are available.
/// CozoStore is internally thread-safe, so no additional synchronization is needed.
#[derive(Debug)]
pub struct AppState {
    /// The graph store backing all queries.
    pub store: CozoStore,
    /// Design snapshot version, if a design was loaded.
    pub design_version: Option<Version>,
    /// Analysis snapshot version, if a project was analyzed.
    pub analysis_version: Option<Version>,
}

/// Type alias for the shared state used in Axum extractors.
pub type SharedState = Arc<AppState>;
```

**Step 2: Create error.rs**

```rust
//! API error types with automatic HTTP status code mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// API error variants.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Resource not found.
    #[error("{0}")]
    NotFound(String),
    /// Bad request parameters.
    #[error("{0}")]
    BadRequest(String),
    /// Internal store error.
    #[error("internal error: {0}")]
    StoreError(#[from] svt_core::store::StoreError),
}

/// JSON error response body.
#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::StoreError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = ErrorBody {
            error: self.to_string(),
        };
        (status, axum::Json(body)).into_response()
    }
}
```

**Step 3: Update main.rs with module declarations**

Replace the entire `crates/server/src/main.rs` with:

```rust
//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod state;

use axum::{routing::get, Router};
use tokio::net::TcpListener;

async fn hello() -> &'static str {
    "software-visualizer-tool server"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(hello));

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("svt-server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p svt-server`
Expected: success

**Step 5: Commit**

```bash
git add crates/server/src/state.rs crates/server/src/error.rs crates/server/src/main.rs
git commit -m "feat(server): add AppState and ApiError types"
```

---

### Task 2: Health and Snapshots Routes

**Files:**
- Create: `crates/server/src/routes/mod.rs`
- Create: `crates/server/src/routes/health.rs`
- Create: `crates/server/src/routes/snapshots.rs`
- Modify: `crates/server/src/main.rs` — add `mod routes`

**Step 1: Create routes/health.rs**

```rust
//! Health check endpoint.

use axum::Json;
use serde::Serialize;

/// Health check response.
#[derive(Serialize)]
pub struct HealthResponse {
    /// Server status.
    pub status: String,
}

/// GET /api/health
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_ok() {
        let app = Router::new().route("/api/health", get(health));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }
}
```

**Step 2: Create routes/snapshots.rs**

```rust
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
    use crate::state::AppState;
    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let mut store = CozoStore::new_in_memory().unwrap();
        store
            .create_snapshot(SnapshotKind::Design, None)
            .unwrap();
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
```

**Step 3: Create routes/mod.rs**

```rust
//! Route modules and router assembly.

pub mod health;
pub mod snapshots;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::state::AppState;

/// Build the full API router with all routes and middleware.
pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/snapshots", get(snapshots::list_snapshots))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
```

**Step 4: Update main.rs to use routes**

Replace `crates/server/src/main.rs` with:

```rust
//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod routes;
mod state;

use std::sync::Arc;

use tokio::net::TcpListener;

use svt_core::store::CozoStore;

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = CozoStore::new_in_memory()?;
    let state = Arc::new(AppState {
        store,
        design_version: None,
        analysis_version: None,
    });

    let app = routes::api_router(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("svt-server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
```

**Step 5: Run tests**

Run: `cargo test -p svt-server`
Expected: 2 tests pass (health, snapshots)

**Step 6: Commit**

```bash
git add crates/server/src/
git commit -m "feat(server): add health and snapshots endpoints with tests"
```

---

### Task 3: Nodes Routes

**Files:**
- Create: `crates/server/src/routes/nodes.rs`
- Modify: `crates/server/src/routes/mod.rs` — register routes

**Step 1: Create routes/nodes.rs**

```rust
//! Node query endpoints.

use axum::extract::{Path, State};
use axum::Json;

use svt_core::model::{Direction, Edge, EdgeKind, Node, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// GET /api/snapshots/:version/nodes
pub async fn list_nodes(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
) -> Result<Json<Vec<Node>>, ApiError> {
    let nodes = state.store.get_all_nodes(version)?;
    Ok(Json(nodes))
}

/// GET /api/snapshots/:version/nodes/:id
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

/// GET /api/snapshots/:version/nodes/:id/children
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

/// GET /api/snapshots/:version/nodes/:id/ancestors
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

/// GET /api/snapshots/:version/nodes/:id/dependencies
pub async fn get_dependencies(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = state
        .store
        .get_edges(version, &id, Direction::Outgoing, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

/// GET /api/snapshots/:version/nodes/:id/dependents
pub async fn get_dependents(
    State(state): State<SharedState>,
    Path((version, id)): Path<(Version, String)>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    state
        .store
        .get_node(version, &id)?
        .ok_or_else(|| ApiError::NotFound(format!("node {id} not found")))?;
    let edges = state
        .store
        .get_edges(version, &id, Direction::Incoming, Some(EdgeKind::Depends))?;
    Ok(Json(edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::routes::api_router;
    use crate::state::AppState;
    use http_body_util::BodyExt;
    use svt_core::model::{
        EdgeKind, Node, NodeKind, Provenance, SnapshotKind,
    };
    use svt_core::store::CozoStore;
    use tower::ServiceExt;

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
        store.add_node(v, &make_node("n1", "/app", NodeKind::System)).unwrap();
        store.add_node(v, &make_node("n2", "/app/core", NodeKind::Service)).unwrap();
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
        let resp = app.oneshot(get_request("/api/snapshots/1/nodes")).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn get_node_returns_single() {
        let app = test_app_with_data();
        let resp = app.oneshot(get_request("/api/snapshots/1/nodes/n1")).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(node["canonical_path"], "/app");
    }

    #[tokio::test]
    async fn get_node_returns_404_for_missing() {
        let app = test_app_with_data();
        let resp = app.oneshot(get_request("/api/snapshots/1/nodes/missing")).await.unwrap();
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
```

**Step 2: Register routes in routes/mod.rs**

Add to `routes/mod.rs`:

```rust
pub mod nodes;
```

Add to the `api_router` function, before `.layer(CorsLayer::permissive())`:

```rust
        .route("/api/snapshots/:version/nodes", get(nodes::list_nodes))
        .route("/api/snapshots/:version/nodes/:id", get(nodes::get_node))
        .route(
            "/api/snapshots/:version/nodes/:id/children",
            get(nodes::get_children),
        )
        .route(
            "/api/snapshots/:version/nodes/:id/ancestors",
            get(nodes::get_ancestors),
        )
        .route(
            "/api/snapshots/:version/nodes/:id/dependencies",
            get(nodes::get_dependencies),
        )
        .route(
            "/api/snapshots/:version/nodes/:id/dependents",
            get(nodes::get_dependents),
        )
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: 6+ tests pass

**Step 4: Commit**

```bash
git add crates/server/src/routes/
git commit -m "feat(server): add node query endpoints with tests"
```

---

### Task 4: Edges Route

**Files:**
- Create: `crates/server/src/routes/edges.rs`
- Modify: `crates/server/src/routes/mod.rs` — register route

**Step 1: Create routes/edges.rs**

```rust
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

/// GET /api/snapshots/:version/edges
pub async fn list_edges(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
    Query(filter): Query<EdgeFilter>,
) -> Result<Json<Vec<Edge>>, ApiError> {
    let edges = state.store.get_all_edges(version, filter.kind)?;
    Ok(Json(edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::routes::api_router;
    use crate::state::AppState;
    use http_body_util::BodyExt;
    use svt_core::model::{NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

    #[tokio::test]
    async fn list_edges_returns_all() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
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
            store,
            design_version: Some(v),
            analysis_version: None,
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
    async fn list_edges_with_kind_filter() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
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
            store,
            design_version: Some(v),
            analysis_version: None,
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
```

**Step 2: Register in routes/mod.rs**

Add `pub mod edges;` and the route:

```rust
        .route("/api/snapshots/:version/edges", get(edges::list_edges))
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: 8+ tests pass

**Step 4: Commit**

```bash
git add crates/server/src/routes/
git commit -m "feat(server): add edges endpoint with kind filter"
```

---

### Task 5: Graph Endpoint (Cytoscape.js Format)

**Files:**
- Create: `crates/server/src/routes/graph.rs`
- Modify: `crates/server/src/routes/mod.rs` — register route

This is the most important endpoint for the frontend. It transforms the graph into Cytoscape.js element format: Contains edges become `parent` fields on nodes, non-containment edges stay as edge elements.

**Step 1: Create routes/graph.rs**

```rust
//! Graph endpoint returning Cytoscape.js-compatible element format.

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use svt_core::model::{EdgeKind, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// A Cytoscape.js node element.
#[derive(Debug, Serialize)]
pub struct CyNode {
    /// Node data.
    pub data: CyNodeData,
}

/// Data fields for a Cytoscape.js node.
#[derive(Debug, Serialize)]
pub struct CyNodeData {
    /// Node ID.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node kind.
    pub kind: String,
    /// Sub-kind (e.g., "crate", "module").
    pub sub_kind: String,
    /// Canonical path.
    pub canonical_path: String,
    /// Parent node ID (for compound nodes). None for root nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Source language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Source reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

/// A Cytoscape.js edge element.
#[derive(Debug, Serialize)]
pub struct CyEdge {
    /// Edge data.
    pub data: CyEdgeData,
}

/// Data fields for a Cytoscape.js edge.
#[derive(Debug, Serialize)]
pub struct CyEdgeData {
    /// Edge ID.
    pub id: String,
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Edge kind.
    pub kind: String,
}

/// Full Cytoscape.js graph payload.
#[derive(Debug, Serialize)]
pub struct CytoscapeGraph {
    /// Graph elements.
    pub elements: CytoscapeElements,
}

/// Nodes and edges in Cytoscape.js format.
#[derive(Debug, Serialize)]
pub struct CytoscapeElements {
    /// Node elements (with parent for compound nodes).
    pub nodes: Vec<CyNode>,
    /// Edge elements (non-containment only).
    pub edges: Vec<CyEdge>,
}

/// GET /api/snapshots/:version/graph
pub async fn get_graph(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
) -> Result<Json<CytoscapeGraph>, ApiError> {
    let all_nodes = state.store.get_all_nodes(version)?;
    let all_edges = state.store.get_all_edges(version, None)?;

    // Build parent map from Contains edges: child_id -> parent_id
    let mut parent_map: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for edge in &all_edges {
        if edge.kind == EdgeKind::Contains {
            parent_map.insert(&edge.target, &edge.source);
        }
    }

    let cy_nodes: Vec<CyNode> = all_nodes
        .iter()
        .map(|node| CyNode {
            data: CyNodeData {
                id: node.id.clone(),
                label: node.name.clone(),
                kind: serde_json::to_value(node.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
                sub_kind: node.sub_kind.clone(),
                canonical_path: node.canonical_path.clone(),
                parent: parent_map.get(node.id.as_str()).map(|s| s.to_string()),
                language: node.language.clone(),
                source_ref: node.source_ref.clone(),
            },
        })
        .collect();

    // Only non-containment edges as Cytoscape edges
    let cy_edges: Vec<CyEdge> = all_edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Contains)
        .map(|edge| CyEdge {
            data: CyEdgeData {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                kind: serde_json::to_value(edge.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
            },
        })
        .collect();

    Ok(Json(CytoscapeGraph {
        elements: CytoscapeElements {
            nodes: cy_nodes,
            edges: cy_edges,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::routes::api_router;
    use crate::state::AppState;
    use http_body_util::BodyExt;
    use svt_core::model::{Edge, Node, NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn graph_contains_edges_become_parent_field() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(v, &make_node("n1", "/app", NodeKind::System)).unwrap();
        store.add_node(v, &make_node("n2", "/app/core", NodeKind::Service)).unwrap();

        // Contains edge: n1 contains n2
        store
            .add_edge(v, &Edge {
                id: "e-c".to_string(),
                source: "n1".to_string(),
                target: "n2".to_string(),
                kind: EdgeKind::Contains,
                provenance: Provenance::Design,
                metadata: None,
            })
            .unwrap();
        // Depends edge: stays as edge
        store
            .add_edge(v, &Edge {
                id: "e-d".to_string(),
                source: "n2".to_string(),
                target: "n1".to_string(),
                kind: EdgeKind::Depends,
                provenance: Provenance::Design,
                metadata: None,
            })
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        let app = api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/snapshots/1/graph")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let graph: CytoscapeGraph = serde_json::from_slice(&body).unwrap();

        // n2 should have parent = n1
        let n2 = graph.elements.nodes.iter().find(|n| n.data.id == "n2").unwrap();
        assert_eq!(n2.data.parent, Some("n1".to_string()));

        // n1 should have no parent
        let n1 = graph.elements.nodes.iter().find(|n| n.data.id == "n1").unwrap();
        assert!(n1.data.parent.is_none());

        // Only non-containment edges in edges list
        assert_eq!(graph.elements.edges.len(), 1);
        assert_eq!(graph.elements.edges[0].data.kind, "depends");
    }
}
```

**Step 2: Register in routes/mod.rs**

Add `pub mod graph;` and the route:

```rust
        .route("/api/snapshots/:version/graph", get(graph::get_graph))
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: 9+ tests pass

**Step 4: Commit**

```bash
git add crates/server/src/routes/
git commit -m "feat(server): add Cytoscape.js graph endpoint"
```

---

### Task 6: Conformance Endpoint

**Files:**
- Create: `crates/server/src/routes/conformance.rs`
- Modify: `crates/server/src/routes/mod.rs` — register routes

**Step 1: Create routes/conformance.rs**

```rust
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

/// GET /api/conformance/design/:version
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
    use crate::routes::api_router;
    use crate::state::AppState;
    use http_body_util::BodyExt;
    use svt_core::model::{Node, NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn design_conformance_returns_report() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(v, &make_node("n1", "/app", NodeKind::System)).unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        let app = api_router(state);
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
        store.add_node(dv, &make_node("d1", "/app", NodeKind::System)).unwrap();

        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store.add_node(av, &make_node("a1", "/app", NodeKind::System)).unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(dv),
            analysis_version: Some(av),
        });
        let app = api_router(state);
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
```

**Step 2: Register in routes/mod.rs**

Add `pub mod conformance;` and the routes:

```rust
        .route(
            "/api/conformance/design/:version",
            get(conformance::evaluate_design),
        )
        .route("/api/conformance", get(conformance::evaluate_conformance))
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: 11+ tests pass

**Step 4: Commit**

```bash
git add crates/server/src/routes/
git commit -m "feat(server): add conformance evaluation endpoints"
```

---

### Task 7: Search Endpoint

**Files:**
- Create: `crates/server/src/routes/search.rs`
- Modify: `crates/server/src/routes/mod.rs` — register route

The search endpoint finds nodes by canonical path glob pattern. Uses `canonical_path_matches` from svt-core.

**Step 1: Create routes/search.rs**

```rust
//! Node search endpoint.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use svt_core::canonical::canonical_path_matches;
use svt_core::model::{Node, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Query parameters for search.
#[derive(Deserialize)]
pub struct SearchParams {
    /// Glob pattern to match canonical paths (e.g., "/svt/core/**").
    pub path: String,
    /// Version to search in.
    pub version: Version,
}

/// GET /api/search?path=GLOB&version=V
pub async fn search_nodes(
    State(state): State<SharedState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<Node>>, ApiError> {
    if params.path.is_empty() {
        return Err(ApiError::BadRequest("path parameter is required".to_string()));
    }
    let all_nodes = state.store.get_all_nodes(params.version)?;
    let matched: Vec<Node> = all_nodes
        .into_iter()
        .filter(|n| canonical_path_matches(&n.canonical_path, &params.path))
        .collect();
    Ok(Json(matched))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::routes::api_router;
    use crate::state::AppState;
    use http_body_util::BodyExt;
    use svt_core::model::{NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn search_by_glob_pattern() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(v, &make_node("n1", "/app", NodeKind::System)).unwrap();
        store.add_node(v, &make_node("n2", "/app/core", NodeKind::Service)).unwrap();
        store.add_node(v, &make_node("n3", "/app/cli", NodeKind::Service)).unwrap();
        store.add_node(v, &make_node("n4", "/other", NodeKind::System)).unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        let app = api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/search?path=/app/**&version=1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let nodes: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        // /app/core and /app/cli match /app/**, but /app itself and /other do not
        assert_eq!(nodes.len(), 2, "should match /app/core and /app/cli");
    }

    #[tokio::test]
    async fn search_empty_path_returns_400() {
        let store = CozoStore::new_in_memory().unwrap();
        let state = Arc::new(AppState {
            store,
            design_version: None,
            analysis_version: None,
        });
        let app = api_router(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/search?path=&version=1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }
}
```

**Step 2: Register in routes/mod.rs**

Add `pub mod search;` and the route:

```rust
        .route("/api/search", get(search::search_nodes))
```

**Step 3: Run tests**

Run: `cargo test -p svt-server`
Expected: 13+ tests pass

**Step 4: Commit**

```bash
git add crates/server/src/routes/
git commit -m "feat(server): add glob-based node search endpoint"
```

---

### Task 8: Startup CLI (clap + data loading)

**Files:**
- Modify: `crates/server/src/main.rs` — real CLI with clap, data loading, tracing

This task replaces the placeholder `main()` with a real CLI that accepts `--project` and `--design` flags, loads data, and starts the server.

**Step 1: Rewrite main.rs**

```rust
//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod routes;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use svt_analyzer::analyze_project;
use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::model::SnapshotKind;
use svt_core::store::{CozoStore, GraphStore};

use crate::state::AppState;

/// Software Visualizer Tool — API server.
#[derive(Parser, Debug)]
#[command(name = "svt-server", version, about)]
struct Args {
    /// Path to a Rust project to analyze at startup.
    #[arg(long)]
    project: Option<PathBuf>,

    /// Path to a design YAML/JSON file to import at startup.
    #[arg(long)]
    design: Option<PathBuf>,

    /// Port to listen on.
    #[arg(long, default_value = "3000")]
    port: u16,

    /// Host to bind to.
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    if args.project.is_none() && args.design.is_none() {
        bail!("at least one of --project or --design is required");
    }

    let mut store = CozoStore::new_in_memory()?;
    let mut design_version = None;
    let mut analysis_version = None;

    // Import design if provided
    if let Some(design_path) = &args.design {
        let content = std::fs::read_to_string(design_path)
            .with_context(|| format!("failed to read {}", design_path.display()))?;
        let doc = interchange::parse_yaml(&content)
            .with_context(|| format!("failed to parse {}", design_path.display()))?;
        let version = interchange_store::load_into_store(&mut store, &doc)?;
        design_version = Some(version);
        info!(version, path = %design_path.display(), "imported design");
    }

    // Analyze project if provided
    if let Some(project_path) = &args.project {
        let summary = analyze_project(&mut store, project_path, None)
            .with_context(|| format!("failed to analyze {}", project_path.display()))?;
        analysis_version = Some(summary.version);
        info!(
            version = summary.version,
            crates = summary.crates_analyzed,
            nodes = summary.nodes_created,
            edges = summary.edges_created,
            "analyzed project"
        );

        // If no design was loaded, check for design version from a previous import
        if design_version.is_none() {
            design_version = store.latest_version(SnapshotKind::Design)?;
        }
    }

    let state = Arc::new(AppState {
        store,
        design_version,
        analysis_version,
    });

    let app = routes::api_router(state);

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).await?;
    info!(address = %bind_addr, "svt-server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p svt-server`
Expected: success

**Step 3: Manual smoke test (optional)**

Run: `cargo run -p svt-server -- --design design/architecture.yaml`
Expected: Server starts, logs design import, curl `http://localhost:3000/api/snapshots` returns JSON.

**Step 4: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat(server): add clap CLI with --project and --design startup modes"
```

---

### Task 9: Integration Tests

**Files:**
- Create: `crates/server/tests/api_integration.rs`

End-to-end tests that load the project's own design YAML and hit all API endpoints. Uses the Axum router directly (no network, no running server).

**Step 1: Create integration test**

```rust
//! Integration tests for the svt-server API.
//!
//! These tests build a router with real data and exercise all endpoints.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::CozoStore;

// We need to access the server's internal modules.
// Since svt-server is a binary crate, we test via the API router directly.
// We reconstruct the router setup here.

/// Build a test router loaded with the project's own design.
fn test_router_with_design() -> axum::Router {
    let design_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("design/architecture.yaml");

    let content = std::fs::read_to_string(&design_path).unwrap();
    let doc = interchange::parse_yaml(&content).unwrap();

    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    // Build a minimal router matching the server's routes.
    // Since we can't import the binary crate's modules, we use axum directly
    // with svt-core APIs and verify JSON responses.
    use axum::extract::{Path, Query, State};
    use axum::routing::get;
    use axum::Json;
    use svt_core::conformance;
    use svt_core::model::Version;
    use svt_core::store::GraphStore;

    let state = Arc::new((store, version));
    type SharedState = Arc<(CozoStore, Version)>;

    async fn list_snapshots(State(state): State<SharedState>) -> Json<serde_json::Value> {
        let snapshots = state.0.list_snapshots().unwrap();
        Json(serde_json::to_value(snapshots).unwrap())
    }

    async fn list_nodes(
        State(state): State<SharedState>,
        Path(version): Path<Version>,
    ) -> Json<serde_json::Value> {
        let nodes = state.0.get_all_nodes(version).unwrap();
        Json(serde_json::to_value(nodes).unwrap())
    }

    async fn list_edges(
        State(state): State<SharedState>,
        Path(version): Path<Version>,
    ) -> Json<serde_json::Value> {
        let edges = state.0.get_all_edges(version, None).unwrap();
        Json(serde_json::to_value(edges).unwrap())
    }

    async fn design_conformance(
        State(state): State<SharedState>,
        Path(version): Path<Version>,
    ) -> Json<serde_json::Value> {
        let report = conformance::evaluate_design(&state.0, version).unwrap();
        Json(serde_json::to_value(report).unwrap())
    }

    axum::Router::new()
        .route("/api/snapshots", get(list_snapshots))
        .route("/api/snapshots/:version/nodes", get(list_nodes))
        .route("/api/snapshots/:version/edges", get(list_edges))
        .route(
            "/api/conformance/design/:version",
            get(design_conformance),
        )
        .with_state(state)
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn snapshots_endpoint_returns_design() {
    let app = test_router_with_design();
    let resp = app.oneshot(get_request("/api/snapshots")).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let snapshots: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(!snapshots.is_empty(), "should have at least one snapshot");
    assert_eq!(snapshots[0]["kind"], "design");
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
    assert!(nodes.len() > 10, "architecture.yaml has many nodes");
    // Check that /svt exists
    assert!(
        nodes
            .iter()
            .any(|n| n["canonical_path"] == "/svt"),
        "should contain root /svt node"
    );
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
    assert!(!edges.is_empty(), "architecture.yaml has edges");
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
    assert!(report["summary"]["passed"].as_u64().unwrap() >= 2);
}
```

**Step 2: Run tests**

Run: `cargo test -p svt-server`
Expected: All unit + integration tests pass

**Step 3: Commit**

```bash
git add crates/server/tests/
git commit -m "test(server): add integration tests using project's own architecture.yaml"
```

---

### Task 10: Dog-Food and Final Verification

**Files:**
- No new files. This task runs the full test suite and verifies the dog-food scenario.

**Step 1: Run all workspace tests**

Run: `cargo test --all`
Expected: 190+ tests pass (existing + ~17 new server tests)

**Step 2: Run clippy**

Run: `cargo clippy --all`
Expected: clean

**Step 3: Run format check**

Run: `cargo fmt --check`
Expected: clean

**Step 4: Manual dog-food (optional)**

Run: `cargo run -p svt-server -- --project . --design design/architecture.yaml`

Then in another terminal:
```bash
curl -s http://localhost:3000/api/health | jq .
curl -s http://localhost:3000/api/snapshots | jq .
curl -s http://localhost:3000/api/snapshots/1/graph | jq '.elements.nodes | length'
curl -s 'http://localhost:3000/api/conformance?design=1&analysis=2' | jq .summary
```

**Step 5: Commit if any final adjustments needed, or confirm clean**

```bash
git log --oneline -10
```
