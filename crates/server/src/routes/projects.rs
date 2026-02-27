//! Project CRUD endpoints.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use svt_core::model::{validate_project_id, Project};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// Request body for creating a project.
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    /// Project ID (slug format).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Response body for a project.
#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    /// Project ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// When the project was created.
    pub created_at: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Extensible metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            created_at: p.created_at,
            description: p.description,
            metadata: p.metadata,
        }
    }
}

/// GET /api/projects
///
/// List all projects in the store.
pub async fn list_projects(
    State(state): State<SharedState>,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let store = state.read_store()?;
    let projects = store.list_projects()?;
    let response: Vec<ProjectResponse> = projects.into_iter().map(ProjectResponse::from).collect();
    Ok(Json(response))
}

/// POST /api/projects
///
/// Create a new project. Returns 400 if the project ID is invalid or already exists.
pub async fn create_project_handler(
    State(state): State<SharedState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(axum::http::StatusCode, Json<ProjectResponse>), ApiError> {
    validate_project_id(&req.id).map_err(ApiError::BadRequest)?;

    let mut store = state.write_store()?;

    if store.project_exists(&req.id)? {
        return Err(ApiError::BadRequest(format!(
            "project '{}' already exists",
            req.id
        )));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let project = Project {
        id: req.id,
        name: req.name,
        created_at: now,
        description: req.description,
        metadata: None,
    };

    store.create_project(&project)?;

    let response = ProjectResponse::from(project);
    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

/// GET /api/projects/{project}
///
/// Get a single project by ID.
pub async fn get_project_handler(
    State(state): State<SharedState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let store = state.read_store()?;
    let project = store
        .get_project(&project_id)?
        .ok_or_else(|| ApiError::NotFound(format!("project '{project_id}' not found")))?;
    Ok(Json(ProjectResponse::from(project)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    use axum::routing::{get, post};
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
            .route(
                "/api/projects",
                get(list_projects).post(create_project_handler),
            )
            .route("/api/projects/{project}", get(get_project_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn list_projects_contains_default() {
        let app = test_app();
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let projects: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        // The "default" project is created automatically by the v1->v2 migration
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["id"], "default");
    }

    #[tokio::test]
    async fn create_and_get_project() {
        let store = CozoStore::new_in_memory().unwrap();
        let state = Arc::new(AppState {
            store: RwLock::new(store),
            default_project: DEFAULT_PROJECT_ID.to_string(),
        });
        let app = Router::new()
            .route("/api/projects", post(create_project_handler))
            .route("/api/projects/{project}", get(get_project_handler))
            .with_state(state);

        // Create
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "id": "my-project",
                            "name": "My Project"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);

        // Get
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/my-project")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let project: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(project["id"], "my-project");
        assert_eq!(project["name"], "My Project");
    }

    #[tokio::test]
    async fn create_project_rejects_invalid_id() {
        let app = test_app();
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/projects")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "id": "INVALID",
                            "name": "Bad Project"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn get_missing_project_returns_404() {
        let app = test_app();
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/nonexistent")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }
}
