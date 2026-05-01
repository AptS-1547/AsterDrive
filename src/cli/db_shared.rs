//! CLI 子命令共用的数据库辅助函数。
//!
//! 这里放置和迁移、doctor 等命令都需要的数据库层小工具，避免每个子模块
//! 各自维护后端命名、迁移历史、标识符转义和连接字符串脱敏逻辑。

use std::collections::HashSet;
use std::path::Path;

use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::errors::{AsterError, MapAsterErr, Result};

pub(super) fn join_strings(values: &[String]) -> String {
    values.join(", ")
}

pub(super) fn backend_name(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::MySql => "mysql",
        DbBackend::Postgres => "postgres",
        DbBackend::Sqlite => "sqlite",
        _ => "unknown",
    }
}

pub(super) fn migration_names() -> Vec<String> {
    Migrator::migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect()
}

pub(super) async fn pending_migrations<C>(
    db: &C,
    backend: DbBackend,
    expected: &[String],
) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    let applied = applied_migrations(db, backend).await?;
    let applied_lookup: HashSet<&str> = applied.iter().map(String::as_str).collect();
    let unknown_applied: Vec<String> = applied
        .iter()
        .filter(|name| !expected.iter().any(|expected_name| expected_name == *name))
        .cloned()
        .collect();
    if !unknown_applied.is_empty() {
        return Err(AsterError::validation_error(format!(
            "database contains unknown migration versions: {}",
            join_strings(&unknown_applied)
        )));
    }

    Ok(expected
        .iter()
        .filter(|name| !applied_lookup.contains(name.as_str()))
        .cloned()
        .collect())
}

pub(super) async fn scalar_i64<C>(db: &C, backend: DbBackend, sql: &str) -> Result<i64>
where
    C: ConnectionTrait,
{
    let row = db
        .query_one_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?
        .ok_or_else(|| AsterError::database_operation(format!("query returned no rows: {sql}")))?;

    if let Ok(value) = row.try_get_by_index::<i64>(0) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get_by_index::<i32>(0) {
        return Ok(i64::from(value));
    }
    if let Ok(value) = row.try_get_by_index::<bool>(0) {
        return Ok(if value { 1 } else { 0 });
    }

    Err(AsterError::database_operation(format!(
        "failed to decode scalar query result as integer: {sql}"
    )))
}

pub(super) fn quote_ident(backend: DbBackend, ident: &str) -> String {
    match backend {
        DbBackend::MySql => format!("`{}`", ident.replace('`', "``")),
        DbBackend::Postgres | DbBackend::Sqlite => {
            format!("\"{}\"", ident.replace('"', "\"\""))
        }
        _ => format!("\"{}\"", ident.replace('"', "\"\"")),
    }
}

pub(super) fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(super) fn quote_sqlite_literal(value: &str) -> String {
    quote_literal(value)
}

pub(super) async fn table_exists<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<bool>
where
    C: ConnectionTrait,
{
    let sql = match backend {
        DbBackend::Sqlite => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        DbBackend::Postgres => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        DbBackend::MySql => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        _ => {
            return Err(AsterError::validation_error(
                "unsupported database backend for table existence checks",
            ));
        }
    };

    scalar_i64(db, backend, &sql).await.map(|value| value != 0)
}

pub(super) fn redact_database_url(database_url: &str) -> String {
    if database_url == "sqlite::memory:" {
        return database_url.to_string();
    }

    if database_url.starts_with("sqlite:") {
        return redact_sqlite_database_url(database_url);
    }

    let Some((scheme, rest)) = database_url.split_once("://") else {
        return database_url.to_string();
    };

    let Some((_authority, suffix)) = rest.rsplit_once('@') else {
        return database_url.to_string();
    };

    format!("{scheme}://***@{suffix}")
}

async fn applied_migrations<C>(db: &C, backend: DbBackend) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    if !table_exists(db, backend, "seaql_migrations").await? {
        return Ok(Vec::new());
    }

    let sql = format!(
        "SELECT {} FROM {} ORDER BY {}",
        quote_ident(backend, "version"),
        quote_ident(backend, "seaql_migrations"),
        quote_ident(backend, "version")
    );
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?;

    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_aster_err(AsterError::database_operation)
        })
        .collect()
}

fn redact_sqlite_database_url(database_url: &str) -> String {
    let Some(path_and_query) = database_url.strip_prefix("sqlite://") else {
        return database_url.to_string();
    };
    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(path, query)| (path, Some(query)));
    let redacted_path = redact_sqlite_path(path);

    match query {
        Some(query) => format!("sqlite://{redacted_path}?{query}"),
        None => format!("sqlite://{redacted_path}"),
    }
}

fn redact_sqlite_path(path: &str) -> String {
    if path == ":memory:" {
        return path.to_string();
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "***".to_string();
    }

    let Some(file_name) = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
    else {
        return "***".to_string();
    };

    if path.starts_with('/') {
        format!("/.../{file_name}")
    } else {
        format!(".../{file_name}")
    }
}

#[cfg(test)]
mod tests {
    use super::redact_database_url;

    #[test]
    fn redact_database_url_masks_network_credentials() {
        assert_eq!(
            redact_database_url("postgres://postgres:postgres@127.0.0.1:5432/asterdrive"),
            "postgres://***@127.0.0.1:5432/asterdrive"
        );
        assert_eq!(
            redact_database_url("mysql://aster@db.internal:3306/asterdrive"),
            "mysql://***@db.internal:3306/asterdrive"
        );
    }

    #[test]
    fn redact_database_url_masks_sqlite_paths_but_preserves_filename() {
        assert_eq!(
            redact_database_url(
                "sqlite:///Users/esap/Desktop/Github/AsterDrive/data/asterdrive.db?mode=rwc"
            ),
            "sqlite:///.../asterdrive.db?mode=rwc"
        );
        assert_eq!(
            redact_database_url("sqlite://data/asterdrive.db?mode=rwc"),
            "sqlite://.../asterdrive.db?mode=rwc"
        );
        assert_eq!(redact_database_url("sqlite::memory:"), "sqlite::memory:");
        assert_eq!(
            redact_database_url("sqlite://:memory:"),
            "sqlite://:memory:"
        );
    }
}
