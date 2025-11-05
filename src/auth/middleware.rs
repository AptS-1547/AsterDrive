use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use super::jwt::AuthError;
use crate::api::AppState;

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"error": "Missing authorization header"})),
            )
                .into_response()
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"error": "Invalid authorization header format"})),
            )
                .into_response()
        })?;

    let claims = state.jwt_manager.verify_token(token).map_err(|e| {
        let error_msg = match e {
            AuthError::TokenExpired => "Token expired",
            _ => "Invalid token",
        };
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"error": error_msg})),
        )
            .into_response()
    })?;

    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}
