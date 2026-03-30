use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, ExprTrait, FromQueryResult,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Expr, Func},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::entities::{
    file::{self, Entity as File},
    file_blob,
    folder::{self, Entity as Folder},
};
use crate::errors::{AsterError, Result};
use sea_orm::sea_query::extension::postgres::PgExpr;

type DateTimeUtc = DateTime<Utc>;

/// Search result file item (includes blob size from JOIN)
#[derive(Debug, Serialize, ToSchema, FromQueryResult)]
pub struct FileSearchItem {
    pub id: i64,
    pub name: String,
    pub folder_id: Option<i64>,
    pub blob_id: i64,
    pub user_id: i64,
    pub mime_type: String,
    pub size: i64,
    #[schema(value_type = String)]
    pub created_at: DateTimeUtc,
    #[schema(value_type = String)]
    pub updated_at: DateTimeUtc,
    pub is_locked: bool,
}

/// Build a case-insensitive LIKE condition using LOWER() for cross-DB compatibility.
/// Escapes `%` and `_` in the search query to prevent wildcard injection.
fn escape_like_query(query: &str) -> String {
    query.replace('%', "\\%").replace('_', "\\_")
}

fn name_like_condition(
    column: impl sea_orm::sea_query::IntoColumnRef + Copy,
    query: &str,
) -> sea_orm::sea_query::SimpleExpr {
    let escaped = escape_like_query(query);
    let pattern = format!("%{escaped}%").to_lowercase();
    Expr::expr(Func::lower(Expr::col(column))).like(pattern)
}

fn mysql_boolean_mode_query(query: &str) -> Option<String> {
    if query.chars().count() < 2 {
        return None;
    }

    let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!("\"{escaped}\""))
}

fn name_search_condition(
    backend: DbBackend,
    column: impl sea_orm::sea_query::IntoColumnRef + Copy,
    query: &str,
) -> sea_orm::sea_query::SimpleExpr {
    match backend {
        DbBackend::Postgres => Expr::col(column).ilike(format!("%{}%", escape_like_query(query))),
        DbBackend::MySql => mysql_boolean_mode_query(query)
            .map(|boolean_query| {
                Expr::cust_with_exprs(
                    "MATCH(?) AGAINST (? IN BOOLEAN MODE)",
                    [Expr::col(column), Expr::val(boolean_query)],
                )
            })
            .unwrap_or_else(|| name_like_condition(column, query)),
        _ => name_like_condition(column, query),
    }
}

/// Search files with optional filters. JOINs file_blobs to include size.
///
/// Returns `(items, total_count)`.
#[allow(clippy::too_many_arguments)]
pub async fn search_files<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    query: Option<&str>,
    mime_type: Option<&str>,
    min_size: Option<i64>,
    max_size: Option<i64>,
    created_after: Option<DateTime<Utc>>,
    created_before: Option<DateTime<Utc>>,
    folder_id: Option<i64>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<FileSearchItem>, u64)> {
    let backend = db.get_database_backend();
    let mut file_condition = Condition::all()
        .add(file::Column::UserId.eq(user_id))
        .add(file::Column::DeletedAt.is_null());
    let mut blob_condition = Condition::all();

    if let Some(q) = query {
        file_condition = file_condition.add(name_search_condition(
            backend,
            (File, file::Column::Name),
            q,
        ));
    }

    if let Some(mt) = mime_type {
        file_condition = file_condition.add(file::Column::MimeType.eq(mt));
    }

    if let Some(min) = min_size {
        blob_condition = blob_condition.add(file_blob::Column::Size.gte(min));
    }

    if let Some(max) = max_size {
        blob_condition = blob_condition.add(file_blob::Column::Size.lte(max));
    }

    if let Some(after) = created_after {
        file_condition = file_condition.add(file::Column::CreatedAt.gte(after));
    }

    if let Some(before) = created_before {
        file_condition = file_condition.add(file::Column::CreatedAt.lte(before));
    }

    if let Some(fid) = folder_id {
        file_condition = file_condition.add(file::Column::FolderId.eq(fid));
    }

    let needs_blob_filters = min_size.is_some() || max_size.is_some();

    let mut count_query = File::find().filter(file_condition.clone());
    if needs_blob_filters {
        count_query = count_query
            .join(JoinType::InnerJoin, file::Relation::FileBlob.def())
            .filter(blob_condition.clone());
    }

    let total = count_query.count(db).await.map_err(AsterError::from)?;

    if total == 0 {
        return Ok((vec![], 0));
    }

    let items = File::find()
        .join(JoinType::InnerJoin, file::Relation::FileBlob.def())
        .filter(file_condition)
        .filter(blob_condition)
        .select_only()
        .column(file::Column::Id)
        .column(file::Column::Name)
        .column(file::Column::FolderId)
        .column(file::Column::BlobId)
        .column(file::Column::UserId)
        .column(file::Column::MimeType)
        .column_as(file_blob::Column::Size, "size")
        .column(file::Column::CreatedAt)
        .column(file::Column::UpdatedAt)
        .column(file::Column::IsLocked)
        .order_by_asc(file::Column::Name)
        .limit(limit)
        .offset(offset)
        .into_model::<FileSearchItem>()
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

/// Search folders with optional filters.
///
/// Returns `(items, total_count)`.
#[allow(clippy::too_many_arguments)]
pub async fn search_folders<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    query: Option<&str>,
    created_after: Option<DateTime<Utc>>,
    created_before: Option<DateTime<Utc>>,
    parent_id: Option<i64>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<folder::Model>, u64)> {
    let backend = db.get_database_backend();
    let mut condition = Condition::all()
        .add(folder::Column::UserId.eq(user_id))
        .add(folder::Column::DeletedAt.is_null());

    if let Some(q) = query {
        condition = condition.add(name_search_condition(
            backend,
            (Folder, folder::Column::Name),
            q,
        ));
    }

    if let Some(after) = created_after {
        condition = condition.add(folder::Column::CreatedAt.gte(after));
    }

    if let Some(before) = created_before {
        condition = condition.add(folder::Column::CreatedAt.lte(before));
    }

    if let Some(pid) = parent_id {
        condition = condition.add(folder::Column::ParentId.eq(pid));
    }

    let base = Folder::find().filter(condition);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;

    if total == 0 {
        return Ok((vec![], 0));
    }

    let items = base
        .order_by_asc(folder::Column::Name)
        .limit(limit)
        .offset(offset)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

#[cfg(test)]
mod tests {
    use super::mysql_boolean_mode_query;
    use sea_orm::{
        DbBackend,
        sea_query::{MysqlQueryBuilder, Query},
    };

    #[test]
    fn mysql_match_against_sql_is_valid() {
        let sql = Query::select()
            .expr(super::name_search_condition(
                DbBackend::MySql,
                super::file::Column::Name,
                "report",
            ))
            .from(super::File)
            .to_string(MysqlQueryBuilder);

        assert!(
            sql.contains(r#"MATCH(`name`) AGAINST ('\"report\"' IN BOOLEAN MODE)"#),
            "{sql}"
        );
        assert!(!sql.contains("$1"), "{sql}");
    }

    #[test]
    fn mysql_boolean_mode_query_uses_phrase_search_for_multi_char_input() {
        assert_eq!(
            mysql_boolean_mode_query("report"),
            Some("\"report\"".into())
        );
        assert_eq!(
            mysql_boolean_mode_query("report\"2026"),
            Some("\"report\\\"2026\"".into())
        );
    }

    #[test]
    fn mysql_boolean_mode_query_falls_back_for_single_character_input() {
        assert_eq!(mysql_boolean_mode_query("r"), None);
    }
}
