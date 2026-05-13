use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TryInsertResult,
};

use crate::entities::file_blob::{self, Entity as FileBlob};
use crate::errors::{AsterError, Result};

use super::ref_count::{find_active_blob_by_hash, increment_blob_ref_count};

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
            let blob_id = existing.id;
            existing.ref_count.checked_add(1).ok_or_else(|| {
                AsterError::internal_error(format!(
                    "file_blob #{} ref_count overflow: {}",
                    existing.id, existing.ref_count
                ))
            })?;
            match increment_blob_ref_count(db, blob_id).await {
                Ok(()) => {
                    return Ok(FindOrCreateBlobResult {
                        model: find_blob_by_id(db, blob_id).await?,
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
            thumbnail_processor: Set(None),
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

pub(super) fn find_or_create_blob_retry_delay(attempt: usize) -> std::time::Duration {
    let backoff_ms = FIND_OR_CREATE_BLOB_INITIAL_DELAY_MS.saturating_mul(1_u64 << attempt.min(4));
    std::time::Duration::from_millis(std::cmp::min(backoff_ms, FIND_OR_CREATE_BLOB_MAX_DELAY_MS))
}

pub async fn find_blob_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file_blob::Model> {
    FileBlob::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("file_blob #{id}")))
}

pub async fn lock_blob_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<file_blob::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => FileBlob::find_by_id(id)
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::record_not_found(format!("file_blob #{id}"))),
        DbBackend::Sqlite => find_blob_by_id(db, id).await,
        _ => find_blob_by_id(db, id).await,
    }
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
        .filter(file_blob::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(blobs.into_iter().map(|b| (b.id, b)).collect())
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
