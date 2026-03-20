use crate::config::DatabaseConfig;
use crate::errors::{AsterError, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub async fn connect(cfg: &DatabaseConfig) -> Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(&cfg.url);
    opt.max_connections(cfg.pool_size)
        .min_connections(1)
        .sqlx_logging(false)
        .test_before_acquire(true);

    let db = Database::connect(opt)
        .await
        .map_err(|e| AsterError::database_operation(e.to_string()))?;

    let backend = db.get_database_backend();
    tracing::info!(backend = ?backend, "database connected");

    if cfg.url.contains("sqlite") {
        tracing::info!("applying SQLite PRAGMA optimizations");
        db.execute_unprepared("PRAGMA journal_mode=WAL;")
            .await
            .map_err(|e| AsterError::database_operation(e.to_string()))?;
        db.execute_unprepared("PRAGMA busy_timeout=5000;")
            .await
            .map_err(|e| AsterError::database_operation(e.to_string()))?;
        db.execute_unprepared("PRAGMA synchronous=NORMAL;")
            .await
            .map_err(|e| AsterError::database_operation(e.to_string()))?;
        db.execute_unprepared("PRAGMA foreign_keys=ON;")
            .await
            .map_err(|e| AsterError::database_operation(e.to_string()))?;
    }

    Ok(db)
}
