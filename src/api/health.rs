use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy")
    ),
    tag = "health"
)]
pub async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "service": "AsterDrive",
            "version": env!("CARGO_PKG_VERSION")
        })),
    )
}
