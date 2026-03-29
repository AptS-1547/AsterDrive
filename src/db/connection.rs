use crate::config::DatabaseConfig;
use crate::errors::{AsterError, MapAsterErr, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub async fn connect(cfg: &DatabaseConfig) -> Result<DatabaseConnection> {
    let is_sqlite = cfg.url.contains("sqlite");
    let max_connections = if is_sqlite { 1 } else { cfg.pool_size };

    let mut opt = ConnectOptions::new(&cfg.url);
    opt.max_connections(max_connections)
        .min_connections(1)
        .sqlx_logging(false)
        .test_before_acquire(true);

    let db = Database::connect(opt)
        .await
        .map_aster_err(AsterError::database_operation)?;

    let backend = db.get_database_backend();
    tracing::info!(backend = ?backend, "database connected");

    if is_sqlite {
        tracing::info!(max_connections, "applying SQLite PRAGMA optimizations");
        db.execute_unprepared("PRAGMA journal_mode=WAL;")
            .await
            .map_aster_err(AsterError::database_operation)?;
        db.execute_unprepared("PRAGMA busy_timeout=15000;")
            .await
            .map_aster_err(AsterError::database_operation)?;
        db.execute_unprepared("PRAGMA synchronous=NORMAL;")
            .await
            .map_aster_err(AsterError::database_operation)?;
        db.execute_unprepared("PRAGMA foreign_keys=ON;")
            .await
            .map_aster_err(AsterError::database_operation)?;
    }

    #[cfg(feature = "metrics")]
    let mut db = db;
    #[cfg(feature = "metrics")]
    install_db_metrics(&mut db);

    Ok(db)
}

#[cfg(feature = "metrics")]
fn install_db_metrics(db: &mut DatabaseConnection) {
    db.set_metric_callback(crate::metrics::record_db_query);
}
