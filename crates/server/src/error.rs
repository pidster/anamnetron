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
    #[allow(dead_code)]
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
