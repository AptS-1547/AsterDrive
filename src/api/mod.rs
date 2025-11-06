pub mod auth;
pub mod dto;
pub mod files;
pub mod health;

use actix_web::web;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{
    auth::{auth_middleware::AuthMiddleware, JwtManager},
    storage::StorageBackend,
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub jwt_manager: JwtManager,
    pub storage: Arc<dyn StorageBackend>,
    pub storage_backend: String,
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health check
        .service(
            web::resource("/health")
                .route(web::get().to(health::health_check))
        )
        // Public routes
        .service(
            web::scope("/api")
                .service(
                    web::scope("/auth")
                        .route("/register", web::post().to(auth::register))
                        .route("/login", web::post().to(auth::login))
                )
                // Protected routes
                .service(
                    web::scope("/files")
                        .wrap(AuthMiddleware)
                        .route("/upload", web::post().to(files::upload_file))
                        .route("", web::get().to(files::list_files))
                        .route("/{id}", web::get().to(files::download_file))
                        .route("/{id}", web::delete().to(files::delete_file))
                )
        );
}
