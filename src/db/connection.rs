use crate::config::DatabaseConfig;
use crate::errors::{AsterError, MapAsterErr, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub async fn connect(cfg: &DatabaseConfig) -> Result<DatabaseConnection> {
    let url = normalize_database_url(&cfg.url);
    let is_sqlite = url.contains("sqlite");
    // SQLite relies on a single pooled connection so concurrent writers are serialized at
    // connection acquisition; repo-layer "lock" helpers do not emulate row locks there.
    let max_connections = if is_sqlite { 1 } else { cfg.pool_size };

    let mut opt = ConnectOptions::new(&url);
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

fn normalize_database_url(database_url: &str) -> String {
    if database_url == "sqlite::memory:" {
        return database_url.to_string();
    }

    if database_url.starts_with("sqlite://") && !database_url.contains('?') {
        return format!("{database_url}?mode=rwc");
    }

    database_url.to_string()
}

#[cfg(feature = "metrics")]
fn install_db_metrics(db: &mut DatabaseConnection) {
    db.set_metric_callback(crate::metrics::record_db_query);
}

#[cfg(test)]
mod tests {
    use super::normalize_database_url;

    #[test]
    fn sqlite_urls_without_query_default_to_rwc_mode() {
        assert_eq!(
            normalize_database_url("sqlite:///var/lib/asterdrive/app.db"),
            "sqlite:///var/lib/asterdrive/app.db?mode=rwc"
        );
        assert_eq!(
            normalize_database_url("sqlite://data/asterdrive.db"),
            "sqlite://data/asterdrive.db?mode=rwc"
        );
    }

    #[test]
    fn sqlite_memory_and_existing_queries_are_preserved() {
        assert_eq!(normalize_database_url("sqlite::memory:"), "sqlite::memory:");
        assert_eq!(
            normalize_database_url("sqlite:///var/lib/asterdrive/app.db?mode=ro"),
            "sqlite:///var/lib/asterdrive/app.db?mode=ro"
        );
        assert_eq!(
            normalize_database_url("postgres://user:pass@localhost/asterdrive"),
            "postgres://user:pass@localhost/asterdrive"
        );
    }
}
