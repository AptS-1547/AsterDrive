use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TryInsertResult, sea_query::Expr,
};

use crate::entities::file_blob::{self, Entity as FileBlob};
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
            thumbnail_path: Set(None),
            thumbnail_version: Set(None),
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

/// 将 blob 的 ref_count 强制重置为 0（用于 reconcile 修正负值）
pub async fn reset_blob_ref_count_to_zero<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileBlob::update_many()
        .col_expr(file_blob::Column::RefCount, Expr::value(0i32))
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 将 blob 的 ref_count 设置为指定值（用于 reconcile 修正偏差）
pub async fn set_blob_ref_count<C: ConnectionTrait>(db: &C, id: i64, ref_count: i32) -> Result<()> {
    FileBlob::update_many()
        .col_expr(file_blob::Column::RefCount, Expr::value(ref_count))
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量扫描 blobs（cursor 分页，id 升序），用于 reconcile 任务
pub async fn find_blobs_paginated<C: ConnectionTrait>(
    db: &C,
    after_id: Option<i64>,
    limit: u64,
) -> Result<Vec<file_blob::Model>> {
    let mut query = FileBlob::find()
        .order_by_asc(file_blob::Column::Id)
        .limit(limit);
    if let Some(last_id) = after_id {
        query = query.filter(file_blob::Column::Id.gt(last_id));
    }
    query.all(db).await.map_err(AsterError::from)
}

/// 统计所有文件（未删除）引用每个 blob 的次数，返回 blob_id → count
pub async fn count_blob_refs_from_files<C: ConnectionTrait>(
    db: &C,
) -> Result<std::collections::HashMap<i64, i64>> {
    use crate::entities::file::{self, Entity as File};
    let rows = File::find()
        .select_only()
        .column(file::Column::BlobId)
        .column_as(Expr::col(file::Column::Id).count(), "ref_count")
        .group_by(file::Column::BlobId)
        .into_tuple::<(i64, i64)>()
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(rows.into_iter().collect())
}

/// 查询存储路径在给定候选集中的所有 blob 路径（用于孤儿检测）
pub async fn find_blob_storage_paths_by_storage_paths<C: ConnectionTrait>(
    db: &C,
    candidate_paths: &[String],
) -> Result<std::collections::HashSet<String>> {
    if candidate_paths.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let paths = FileBlob::find()
        .select_only()
        .column(file_blob::Column::StoragePath)
        .filter(file_blob::Column::StoragePath.is_in(candidate_paths.iter().cloned()))
        .into_tuple::<String>()
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(paths.into_iter().collect())
}

pub async fn set_thumbnail_metadata<C: ConnectionTrait>(
    db: &C,
    id: i64,
    thumbnail_path: &str,
    thumbnail_version: &str,
) -> Result<bool> {
    let result = FileBlob::update_many()
        .col_expr(
            file_blob::Column::ThumbnailPath,
            Expr::value(Some(thumbnail_path.to_string())),
        )
        .col_expr(
            file_blob::Column::ThumbnailVersion,
            Expr::value(Some(thumbnail_version.to_string())),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn clear_thumbnail_metadata<C: ConnectionTrait>(db: &C, id: i64) -> Result<bool> {
    let result = FileBlob::update_many()
        .col_expr(
            file_blob::Column::ThumbnailPath,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            file_blob::Column::ThumbnailVersion,
            Expr::value(Option::<String>::None),
        )
        .col_expr(file_blob::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(file_blob::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

/// 统计 blob 总数
pub async fn count_all_blobs<C: ConnectionTrait>(db: &C) -> Result<u64> {
    FileBlob::find().count(db).await.map_err(AsterError::from)
}

/// 统计所有 blob 的总字节数
pub async fn sum_blob_bytes<C: ConnectionTrait>(db: &C) -> Result<i64> {
    let type_name = match db.get_database_backend() {
        DbBackend::Postgres => "bigint",
        DbBackend::MySql => "signed",
        _ => "integer",
    };
    Ok(FileBlob::find()
        .select_only()
        .column_as(
            Expr::col(file_blob::Column::Size).sum().cast_as(type_name),
            "sum",
        )
        .into_tuple::<Option<i64>>()
        .one(db)
        .await?
        .flatten()
        .unwrap_or(0))
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
            thumbnail_path: Set(None),
            thumbnail_version: Set(None),
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
