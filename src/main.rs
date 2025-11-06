mod api;
mod auth;
mod config;
mod db;
mod models;
mod storage;

use actix_cors::Cors;
use actix_web::{middleware as actix_middleware, web, App, HttpServer};
use anyhow::Result;
use migration::{Migrator, MigratorTrait};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    api::{configure_routes, AppState},
    auth::JwtManager,
    config::Config,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "asterdrive=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded");

    // Connect to database
    let db = db::establish_connection(&config.database.url).await?;
    info!("Database connected");

    // Run migrations
    info!("Running database migrations...");
    Migrator::up(&db, None).await?;
    info!("Database migrations completed");

    // Initialize storage backend
    let storage = storage::create_storage_backend(&config.storage).await?;
    info!("Storage backend initialized: {}", config.storage.backend);

    // Initialize JWT manager
    let jwt_manager = JwtManager::new(
        config.jwt.secret.clone(),
        config.jwt.expiration_hours,
    );

    // Create application state
    let state = AppState {
        db,
        jwt_manager,
        storage,
        storage_backend: config.storage.backend.clone(),
    };

    // Server address
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Starting server on {}", addr);

    // Start HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(actix_middleware::Logger::default())
            .wrap(cors)
            .configure(configure_routes)
    })
    .bind(&addr)?
    .run()
    .await?;

    Ok(())
}
