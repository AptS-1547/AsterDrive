use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, ExprTrait, FromQueryResult, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
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
fn name_like_condition<E: EntityTrait>(
    column: impl sea_orm::sea_query::IntoColumnRef + Copy,
    query: &str,
) -> sea_orm::sea_query::SimpleExpr {
    let escaped = query.replace('%', "\\%").replace('_', "\\_");
    let pattern = format!("%{escaped}%").to_lowercase();
    Expr::expr(Func::lower(Expr::col(column))).like(pattern)
}

/// Search files with optional filters. JOINs file_blobs to include size.
///
/// Returns `(items, total_count)`.
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
    let mut condition = Condition::all()
        .add(file::Column::UserId.eq(user_id))
        .add(file::Column::DeletedAt.is_null());

    if let Some(q) = query {
        condition = condition.add(name_like_condition::<File>((File, file::Column::Name), q));
    }

    if let Some(mt) = mime_type {
        condition = condition.add(file::Column::MimeType.eq(mt));
    }

    if let Some(min) = min_size {
        condition = condition.add(file_blob::Column::Size.gte(min));
    }

    if let Some(max) = max_size {
        condition = condition.add(file_blob::Column::Size.lte(max));
    }

    if let Some(after) = created_after {
        condition = condition.add(file::Column::CreatedAt.gte(after));
    }

    if let Some(before) = created_before {
        condition = condition.add(file::Column::CreatedAt.lte(before));
    }

    if let Some(fid) = folder_id {
        condition = condition.add(file::Column::FolderId.eq(fid));
    }

    // Always JOIN file_blobs: needed for size filters and for the size column in results
    let base = File::find()
        .join(JoinType::InnerJoin, file::Relation::FileBlob.def())
        .filter(condition);

    let total = base.clone().count(db).await.map_err(AsterError::from)?;

    if total == 0 {
        return Ok((vec![], 0));
    }

    let items = base
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
    let mut condition = Condition::all()
        .add(folder::Column::UserId.eq(user_id))
        .add(folder::Column::DeletedAt.is_null());

    if let Some(q) = query {
        condition = condition.add(name_like_condition::<Folder>(
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
