pub mod auth;
pub mod dto;
pub mod files;

use axum::{
    middleware,
    routing::{get, post, delete},
    Router,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::{auth_middleware, JwtManager},
    storage::StorageBackend,
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub jwt_manager: JwtManager,
    pub storage: Arc<dyn StorageBackend>,
    pub storage_backend: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        auth::register,
        auth::login,
        files::upload_file,
        files::list_files,
        files::download_file,
        files::delete_file,
    ),
    components(
        schemas(
            dto::RegisterRequest,
            dto::LoginRequest,
            dto::AuthResponse,
            dto::UserResponse,
            dto::FileUploadResponse,
            dto::FileListResponse,
            dto::FileInfo,
            dto::ErrorResponse,
        )
    ),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "files", description = "File management endpoints")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            )
        }
    }
}

pub fn create_router(state: AppState) -> Router {
    // Public routes
    let public_routes = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login));

    // Protected routes - need to pass state for middleware
    let protected_routes = Router::new()
        .route("/files/upload", post(files::upload_file))
        .route("/files", get(files::list_files))
        .route("/files/:id", get(files::download_file))
        .route("/files/:id", delete(files::delete_file))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .nest("/api", public_routes.merge(protected_routes))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
