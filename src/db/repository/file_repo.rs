use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TryInsertResult, sea_query::Expr,
};

use crate::entities::{
    file::{self, Entity as File},
    file_blob::{self, Entity as FileBlob},
};
use crate::errors::{AsterError, Result};

pub struct FindOrCreateBlobResult {
    pub model: file_blob::Model,
    pub inserted: bool,
}

// `find_or_create_blob()` only retries short-lived races:
// 1. another transaction inserted the same (hash, policy_id) row but has not become visible yet;
// 2. a cleanup worker deleted a zero-ref blob after we read it but before we bumped ref_count.
//
// Those windows should resolve after the competing transaction commits, so we use a small
// exponential backoff budget instead of a fixed 1s spin loop. Total sleep is capped at
// 5 + 10 + 20 + 40 + 80 + 80 = 235ms across 7 attempts.
const FIND_OR_CREATE_BLOB_MAX_ATTEMPTS: usize = 7;
const FIND_OR_CREATE_BLOB_INITIAL_DELAY_MS: u64 = 5;
const FIND_OR_CREATE_BLOB_MAX_DELAY_MS: u64 = 80;

#[derive(Clone, Copy)]
enum FileScope {
    Personal { user_id: i64 },
    Team { team_id: i64 },
}

fn scope_condition(scope: FileScope) -> Condition {
    match scope {
        FileScope::Personal { user_id } => Condition::all()
            .add(file::Column::UserId.eq(user_id))
            .add(file::Column::TeamId.is_null()),
        FileScope::Team { team_id } => Condition::all().add(file::Column::TeamId.eq(team_id)),
    }
}

fn active_scope_condition(scope: FileScope) -> Condition {
    scope_condition(scope).add(file::Column::DeletedAt.is_null())
}

fn apply_folder_condition(cond: Condition, folder_id: Option<i64>) -> Condition {
    match folder_id {
        Some(folder_id) => cond.add(file::Column::FolderId.eq(folder_id)),
        None => cond.add(file::Column::FolderId.is_null()),
    }
}

async fn find_by_folders_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    if folder_ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(active_scope_condition(scope))
        .filter(file::Column::FolderId.is_in(folder_ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

async fn find_by_folder_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(apply_folder_condition(
            active_scope_condition(scope),
            folder_id,
        ))
        .order_by_asc(file::Column::Name)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file::Model> {
    File::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::file_not_found(format!("file #{id}")))
}

pub async fn lock_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => File::find_by_id(id)
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::file_not_found(format!("file #{id}"))),
        DbBackend::Sqlite => {
            File::update_many()
                .col_expr(file::Column::UpdatedAt, Expr::col(file::Column::UpdatedAt))
                .filter(file::Column::Id.eq(id))
                .exec(db)
                .await
                .map_err(AsterError::from)?;
            find_by_id(db, id).await
        }
        _ => find_by_id(db, id).await,
    }
}

pub async fn find_by_ids<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<Vec<file::Model>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(file::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 批量查询多个文件夹下的未删除文件
pub async fn find_by_folders<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_folders_in_scope(db, FileScope::Personal { user_id }, folder_ids).await
}

pub async fn find_by_team_folders<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    find_by_folders_in_scope(db, FileScope::Team { team_id }, folder_ids).await
}

/// 批量查询多个文件夹下的文件（含已删除）
pub async fn find_all_in_folders<C: ConnectionTrait>(
    db: &C,
    folder_ids: &[i64],
) -> Result<Vec<file::Model>> {
    if folder_ids.is_empty() {
        return Ok(vec![]);
    }
    File::find()
        .filter(file::Column::FolderId.is_in(folder_ids.to_vec()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询文件夹下的文件（排除已删除）
pub async fn find_by_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    find_by_folder_in_scope(db, FileScope::Personal { user_id }, folder_id).await
}

pub async fn find_by_team_folder<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    find_by_folder_in_scope(db, FileScope::Team { team_id }, folder_id).await
}

/// 查询文件夹下的文件（排除已删除，cursor 分页，支持多字段排序）
async fn find_by_folder_cursor_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    use crate::api::pagination::{SortBy, SortOrder};

    let base = File::find().filter(apply_folder_condition(
        active_scope_condition(scope),
        folder_id,
    ));
    let total = base.clone().count(db).await.map_err(AsterError::from)?;

    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let is_asc = matches!(sort_order, SortOrder::Asc);

    let mut q = base;
    if let Some((after_value, after_id)) = after {
        let cursor_cond = build_cursor_condition(sort_by, is_asc, &after_value, after_id)?;
        q = q.filter(cursor_cond);
    }

    let primary_col = match sort_by {
        SortBy::Name => file::Column::Name,
        SortBy::Size => file::Column::Size,
        SortBy::CreatedAt => file::Column::CreatedAt,
        SortBy::UpdatedAt => file::Column::UpdatedAt,
        SortBy::Type => file::Column::MimeType,
    };

    q = if is_asc {
        q.order_by_asc(primary_col).order_by_asc(file::Column::Id)
    } else {
        q.order_by_desc(primary_col).order_by_desc(file::Column::Id)
    };

    let items = q.limit(limit).all(db).await.map_err(AsterError::from)?;
    Ok((items, total))
}

pub async fn find_by_folder_cursor<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    find_by_folder_cursor_in_scope(
        db,
        FileScope::Personal { user_id },
        folder_id,
        limit,
        after,
        sort_by,
        sort_order,
    )
    .await
}

pub async fn find_by_team_folder_cursor<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    limit: u64,
    after: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<(Vec<file::Model>, u64)> {
    find_by_folder_cursor_in_scope(
        db,
        FileScope::Team { team_id },
        folder_id,
        limit,
        after,
        sort_by,
        sort_order,
    )
    .await
}

/// 构建 cursor WHERE 条件
/// ASC:  (col > val) OR (col = val AND id > after_id)
/// DESC: (col < val) OR (col = val AND id < after_id)
fn build_cursor_condition(
    sort_by: crate::api::pagination::SortBy,
    is_asc: bool,
    after_value: &str,
    after_id: i64,
) -> Result<sea_orm::Condition> {
    use crate::api::pagination::SortBy;

    let id_cond = if is_asc {
        file::Column::Id.gt(after_id)
    } else {
        file::Column::Id.lt(after_id)
    };

    match sort_by {
        SortBy::Name => {
            let val = after_value.to_string();
            let (gt, eq) = if is_asc {
                (
                    file::Column::Name.gt(val.clone()),
                    file::Column::Name.eq(val),
                )
            } else {
                (
                    file::Column::Name.lt(val.clone()),
                    file::Column::Name.eq(val),
                )
            };
            Ok(sea_orm::Condition::any()
                .add(gt)
                .add(sea_orm::Condition::all().add(eq).add(id_cond)))
        }
        SortBy::Size => {
            let val: i64 = after_value
                .parse()
                .map_err(|_| AsterError::validation_error("invalid cursor value for size sort"))?;
            let (gt, eq) = if is_asc {
                (file::Column::Size.gt(val), file::Column::Size.eq(val))
            } else {
                (file::Column::Size.lt(val), file::Column::Size.eq(val))
            };
            Ok(sea_orm::Condition::any()
                .add(gt)
                .add(sea_orm::Condition::all().add(eq).add(id_cond)))
        }
        SortBy::CreatedAt => {
            let val: chrono::DateTime<chrono::Utc> = after_value.parse().map_err(|_| {
                AsterError::validation_error("invalid cursor value for created_at sort")
            })?;
            let (gt, eq) = if is_asc {
                (
                    file::Column::CreatedAt.gt(val),
                    file::Column::CreatedAt.eq(val),
                )
            } else {
                (
                    file::Column::CreatedAt.lt(val),
                    file::Column::CreatedAt.eq(val),
                )
            };
            Ok(sea_orm::Condition::any()
                .add(gt)
                .add(sea_orm::Condition::all().add(eq).add(id_cond)))
        }
        SortBy::UpdatedAt => {
            let val: chrono::DateTime<chrono::Utc> = after_value.parse().map_err(|_| {
                AsterError::validation_error("invalid cursor value for updated_at sort")
            })?;
            let (gt, eq) = if is_asc {
                (
                    file::Column::UpdatedAt.gt(val),
                    file::Column::UpdatedAt.eq(val),
                )
            } else {
                (
                    file::Column::UpdatedAt.lt(val),
                    file::Column::UpdatedAt.eq(val),
                )
            };
            Ok(sea_orm::Condition::any()
                .add(gt)
                .add(sea_orm::Condition::all().add(eq).add(id_cond)))
        }
        SortBy::Type => {
            let val = after_value.to_string();
            let (gt, eq) = if is_asc {
                (
                    file::Column::MimeType.gt(val.clone()),
                    file::Column::MimeType.eq(val),
                )
            } else {
                (
                    file::Column::MimeType.lt(val.clone()),
                    file::Column::MimeType.eq(val),
                )
            };
            Ok(sea_orm::Condition::any()
                .add(gt)
                .add(sea_orm::Condition::all().add(eq).add(id_cond)))
        }
    }
}

/// 查询顶层已删除文件（cursor 分页），cursor = (deleted_at, id) 降序
fn top_level_deleted_condition(scope: FileScope) -> Condition {
    use sea_orm::sea_query::{Alias, Expr, Query};

    let folder_deleted_subquery = Query::select()
        .expr(Expr::val(1i32))
        .from_as(Alias::new("folders"), Alias::new("f2"))
        .and_where(
            Expr::col((Alias::new("f2"), Alias::new("id")))
                .equals((Alias::new("files"), file::Column::FolderId)),
        )
        .and_where(Expr::col((Alias::new("f2"), Alias::new("deleted_at"))).is_not_null())
        .to_owned();

    scope_condition(scope)
        .add(file::Column::DeletedAt.is_not_null())
        .add(
            Condition::any()
                .add(file::Column::FolderId.is_null())
                .add(Expr::exists(folder_deleted_subquery).not()),
        )
}

async fn find_top_level_deleted_paginated_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    limit: u64,
    after: Option<(chrono::DateTime<Utc>, i64)>,
) -> Result<(Vec<file::Model>, u64)> {
    let base_cond = top_level_deleted_condition(scope);
    let base = File::find().filter(base_cond.clone());

    let total = base.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let mut q = File::find().filter(base_cond);
    if let Some((after_deleted_at, after_id)) = after {
        q = q.filter(
            Condition::any()
                .add(file::Column::DeletedAt.lt(after_deleted_at))
                .add(
                    Condition::all()
                        .add(file::Column::DeletedAt.eq(after_deleted_at))
                        .add(file::Column::Id.gt(after_id)),
                ),
        );
    }

    let items = q
        .order_by_desc(file::Column::DeletedAt)
        .order_by_asc(file::Column::Id)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

pub async fn find_top_level_deleted_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    after: Option<(chrono::DateTime<chrono::Utc>, i64)>,
) -> Result<(Vec<file::Model>, u64)> {
    find_top_level_deleted_paginated_in_scope(db, FileScope::Personal { user_id }, limit, after)
        .await
}

pub async fn find_top_level_deleted_by_team_paginated<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    limit: u64,
    after: Option<(chrono::DateTime<Utc>, i64)>,
) -> Result<(Vec<file::Model>, u64)> {
    find_top_level_deleted_paginated_in_scope(db, FileScope::Team { team_id }, limit, after).await
}

/// 按名称查文件（排除已删除）
async fn find_by_name_in_folder_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    File::find()
        .filter(apply_folder_condition(
            active_scope_condition(scope),
            folder_id,
        ))
        .filter(file::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_name_in_folder<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    find_by_name_in_folder_in_scope(db, FileScope::Personal { user_id }, folder_id, name).await
}

pub async fn find_by_name_in_team_folder<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    find_by_name_in_folder_in_scope(db, FileScope::Team { team_id }, folder_id, name).await
}

/// 查找不冲突的文件名：如果 name 已存在则递增 " (1)", " (2)" ...
async fn resolve_unique_filename_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: FileScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    let mut final_name = name.to_string();
    while find_by_name_in_folder_in_scope(db, scope, folder_id, &final_name)
        .await?
        .is_some()
    {
        final_name = crate::utils::next_copy_name(&final_name);
    }
    Ok(final_name)
}

pub async fn resolve_unique_filename<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    resolve_unique_filename_in_scope(db, FileScope::Personal { user_id }, folder_id, name).await
}

pub async fn resolve_unique_team_filename<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    resolve_unique_filename_in_scope(db, FileScope::Team { team_id }, folder_id, name).await
}

pub async fn find_blob_by_hash<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    policy_id: i64,
) -> Result<Option<file_blob::Model>> {
    FileBlob::find()
        .filter(file_blob::Column::Hash.eq(hash))
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_active_blob_by_hash<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    policy_id: i64,
) -> Result<Option<file_blob::Model>> {
    FileBlob::find()
        .filter(file_blob::Column::Hash.eq(hash))
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .filter(file_blob::Column::RefCount.gte(0))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create_blob<C: ConnectionTrait>(
    db: &C,
    model: file_blob::ActiveModel,
) -> Result<file_blob::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// Blob 去重：查找已有 blob 则原子递增 ref_count 并返回，否则新建 ref_count=1。
pub async fn find_or_create_blob<C: ConnectionTrait>(
    db: &C,
    hash: &str,
    size: i64,
    policy_id: i64,
    storage_path: &str,
) -> Result<FindOrCreateBlobResult> {
    for attempt in 0..FIND_OR_CREATE_BLOB_MAX_ATTEMPTS {
        if let Some(existing) = find_active_blob_by_hash(db, hash, policy_id).await? {
            match increment_blob_ref_count(db, existing.id).await {
                Ok(()) => {
                    return Ok(FindOrCreateBlobResult {
                        model: find_blob_by_id(db, existing.id).await?,
                        inserted: false,
                    });
                }
                Err(e) if e.code() == "E006" => {
                    if attempt + 1 == FIND_OR_CREATE_BLOB_MAX_ATTEMPTS {
                        break;
                    }
                    tokio::time::sleep(find_or_create_blob_retry_delay(attempt)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        let now = Utc::now();
        let inserted = match FileBlob::insert(file_blob::ActiveModel {
            hash: Set(hash.to_string()),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path.to_string()),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .on_conflict_do_nothing_on([file_blob::Column::Hash, file_blob::Column::PolicyId])
        .exec(db)
        .await
        .map_err(AsterError::from)?
        {
            TryInsertResult::Inserted(_) => true,
            TryInsertResult::Conflicted => false,
            TryInsertResult::Empty => {
                return Err(AsterError::internal_error(
                    "find_or_create_blob produced empty insert result",
                ));
            }
        };

        if inserted {
            return Ok(FindOrCreateBlobResult {
                model: find_blob_by_hash(db, hash, policy_id).await?.ok_or_else(|| {
                    AsterError::internal_error(format!(
                        "find_or_create_blob could not reload inserted blob for hash={hash}, policy_id={policy_id}"
                    ))
                })?,
                inserted: true,
            });
        }

        if attempt + 1 == FIND_OR_CREATE_BLOB_MAX_ATTEMPTS {
            break;
        }
        tokio::time::sleep(find_or_create_blob_retry_delay(attempt)).await;
    }

    Err(AsterError::internal_error(format!(
        "find_or_create_blob exceeded contention retry budget after {FIND_OR_CREATE_BLOB_MAX_ATTEMPTS} attempts for hash={hash}, policy_id={policy_id}"
    )))
}

fn find_or_create_blob_retry_delay(attempt: usize) -> std::time::Duration {
    let backoff_ms = FIND_OR_CREATE_BLOB_INITIAL_DELAY_MS.saturating_mul(1_u64 << attempt.min(4));
    std::time::Duration::from_millis(std::cmp::min(backoff_ms, FIND_OR_CREATE_BLOB_MAX_DELAY_MS))
}

/// 原子递增 blob ref_count（防止并发丢更新）
pub async fn increment_blob_ref_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let result = FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::col(file_blob::Column::RefCount).add(1i32),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .filter(file_blob::Column::RefCount.gte(0))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("file_blob #{id}")));
    }
    Ok(())
}

/// 原子增加 blob ref_count（可变增量，批量复制用）
pub async fn increment_blob_ref_count_by<C: ConnectionTrait>(
    db: &C,
    id: i64,
    delta: i32,
) -> Result<()> {
    if delta < 0 {
        return Err(AsterError::internal_error(format!(
            "increment_blob_ref_count_by requires positive delta, got {delta}"
        )));
    }
    if delta == 0 {
        return Ok(());
    }
    let result = FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::col(file_blob::Column::RefCount).add(delta),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .filter(file_blob::Column::RefCount.gte(0))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("file_blob #{id}")));
    }
    Ok(())
}

/// 原子递减 blob ref_count（floor 0，防止并发丢更新）
pub async fn decrement_blob_ref_count<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::case(Expr::col(file_blob::Column::RefCount).lt(1i32), 0)
                .finally(Expr::col(file_blob::Column::RefCount).sub(1i32))
                .into(),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 原子递减 blob ref_count（可变减量，floor 0）
pub async fn decrement_blob_ref_count_by<C: ConnectionTrait>(
    db: &C,
    id: i64,
    delta: i32,
) -> Result<()> {
    if delta < 0 {
        return Err(AsterError::internal_error(format!(
            "decrement_blob_ref_count_by requires positive delta, got {delta}"
        )));
    }
    if delta == 0 {
        return Ok(());
    }
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::case(Expr::col(file_blob::Column::RefCount).lt(delta), 0)
                .finally(Expr::col(file_blob::Column::RefCount).sub(delta))
                .into(),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 统计某存储策略下的 blob 数量（策略删除保护用）
pub async fn count_blobs_by_policy<C: ConnectionTrait>(db: &C, policy_id: i64) -> Result<u64> {
    FileBlob::find()
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(db: &C, model: file::ActiveModel) -> Result<file::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 批量插入文件记录（不返回创建的 Model，批量复制用）
pub async fn create_many<C: ConnectionTrait>(db: &C, models: Vec<file::ActiveModel>) -> Result<()> {
    if models.is_empty() {
        return Ok(());
    }
    File::insert_many(models)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量移动文件到同一文件夹
pub async fn move_many_to_folder<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    folder_id: Option<i64>,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::update_many()
        .col_expr(file::Column::FolderId, Expr::value(folder_id))
        .col_expr(file::Column::UpdatedAt, Expr::value(now))
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn find_blob_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file_blob::Model> {
    FileBlob::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("file_blob #{id}")))
}

/// 批量查询 blob，返回 id → Model 的映射
pub async fn find_blobs_by_ids<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, file_blob::Model>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let blobs = FileBlob::find()
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(blobs.into_iter().map(|b| (b.id, b)).collect())
}

/// 硬删除文件记录（回收站清理用）
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    File::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量硬删除文件记录
pub async fn delete_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::delete_many()
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量硬删除 blob 记录
pub async fn delete_blobs<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    FileBlob::delete_many()
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量原子递减 blob ref_count
pub async fn decrement_blob_ref_counts<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    FileBlob::update_many()
        .col_expr(
            file_blob::Column::RefCount,
            Expr::case(Expr::col(file_blob::Column::RefCount).lt(1i32), 0)
                .finally(Expr::col(file_blob::Column::RefCount).sub(1i32))
                .into(),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_blob<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn claim_blob_cleanup<C: ConnectionTrait>(db: &C, id: i64) -> Result<bool> {
    let result = FileBlob::update_many()
        .col_expr(file_blob::Column::RefCount, Expr::value(-1i32))
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .filter(file_blob::Column::RefCount.eq(0))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn restore_blob_cleanup_claim<C: ConnectionTrait>(db: &C, id: i64) -> Result<bool> {
    let result = FileBlob::update_many()
        .col_expr(file_blob::Column::RefCount, Expr::value(0i32))
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .filter(file_blob::Column::RefCount.eq(-1))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn delete_blob_if_cleanup_claimed<C: ConnectionTrait>(db: &C, id: i64) -> Result<bool> {
    let result = FileBlob::delete_many()
        .filter(file_blob::Column::Id.eq(id))
        .filter(file_blob::Column::RefCount.eq(-1))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

// ── 软删除 / 回收站 ─────────────────────────────────────────────────

/// 软删除：标记 deleted_at
pub async fn soft_delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量软删除：一次 UPDATE 标记多个文件的 deleted_at
pub async fn soft_delete_many<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::update_many()
        .col_expr(file::Column::DeletedAt, Expr::value(Some(now)))
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 恢复：清除 deleted_at
pub async fn restore<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(None);
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量恢复：一次 UPDATE 清除多个文件的 deleted_at
pub async fn restore_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    File::update_many()
        .col_expr(
            file::Column::DeletedAt,
            Expr::value(Option::<chrono::DateTime<Utc>>::None),
        )
        .filter(file::Column::Id.is_in(ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 查询用户回收站中的文件
pub async fn find_deleted_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::TeamId.is_null())
        .filter(file::Column::DeletedAt.is_not_null())
        .order_by_desc(file::Column::DeletedAt)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询某文件夹下的已删除文件（递归恢复/清理用，避免 N+1）
pub async fn find_deleted_in_folder<C: ConnectionTrait>(
    db: &C,
    folder_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::FolderId.eq(folder_id))
        .filter(file::Column::DeletedAt.is_not_null())
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询过期的已删除文件（自动清理用）
pub async fn find_expired_deleted<C: ConnectionTrait>(
    db: &C,
    before: chrono::DateTime<Utc>,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::DeletedAt.is_not_null())
        .filter(file::Column::DeletedAt.lt(before))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询用户的所有文件（含已删除，force_delete 用）
pub async fn find_all_by_user<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::TeamId.is_null())
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all_by_team<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::TeamId.eq(team_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};
    use std::time::Duration;

    #[test]
    fn find_or_create_blob_retry_delay_grows_exponentially_and_caps() {
        assert_eq!(find_or_create_blob_retry_delay(0), Duration::from_millis(5));
        assert_eq!(
            find_or_create_blob_retry_delay(1),
            Duration::from_millis(10)
        );
        assert_eq!(
            find_or_create_blob_retry_delay(2),
            Duration::from_millis(20)
        );
        assert_eq!(
            find_or_create_blob_retry_delay(3),
            Duration::from_millis(40)
        );
        assert_eq!(
            find_or_create_blob_retry_delay(4),
            Duration::from_millis(80)
        );
        assert_eq!(
            find_or_create_blob_retry_delay(5),
            Duration::from_millis(80)
        );
        assert_eq!(
            find_or_create_blob_retry_delay(99),
            Duration::from_millis(80)
        );
    }

    #[test]
    fn postgres_find_or_create_blob_insert_sql_uses_valid_on_conflict() {
        let now = Utc::now();
        let sql = FileBlob::insert(file_blob::ActiveModel {
            hash: Set("hash".to_string()),
            size: Set(1),
            policy_id: Set(2),
            storage_path: Set("files/hash".to_string()),
            ref_count: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .on_conflict_do_nothing_on([file_blob::Column::Hash, file_blob::Column::PolicyId])
        .build(DbBackend::Postgres)
        .to_string();

        assert!(
            sql.contains(r#"ON CONFLICT ("hash", "policy_id") DO NOTHING"#),
            "{sql}"
        );
        assert!(!sql.contains(" WHERE "), "{sql}");
    }
}
