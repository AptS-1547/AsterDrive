mod api;
mod auth;
mod config;
mod db;
mod models;
mod storage;

use anyhow::Result;
use migration::{Migrator, MigratorTrait};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    api::{create_router, AppState},
    auth::JwtManager,
    config::Config,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "asterdrive=debug,tower_http=debug".into()),
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

    // Create router
    let app = create_router(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Server listening on {}", addr);
    info!("API documentation available at http://{}/swagger-ui", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
