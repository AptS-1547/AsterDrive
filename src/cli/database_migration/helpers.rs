//! CLI 子模块：`helpers`。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::errors::{AsterError, MapAsterErr, Result};

pub(super) fn join_strings(values: &[String]) -> String {
    values.join(", ")
}

pub(super) async fn count_rows<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<i64>
where
    C: ConnectionTrait,
{
    scalar_i64(
        db,
        backend,
        &format!("SELECT COUNT(*) FROM {}", quote_ident(backend, table_name)),
    )
    .await
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

pub(super) fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_millis(),
    )
    .expect("timestamp milliseconds should fit into i64")
}

pub(super) fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_timestamp(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

pub(super) fn nullable_sql_string(value: Option<&str>) -> String {
    value
        .map(quote_literal)
        .unwrap_or_else(|| "NULL".to_string())
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
    }
}
