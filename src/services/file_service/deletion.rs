//! 文件服务子模块：`deletion`。

use futures::{StreamExt, stream};

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    storage_change_service, thumbnail_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::utils::numbers::usize_to_u32;

use super::get_info_in_scope;

const BLOB_CLEANUP_CONCURRENCY: usize = 8;

pub(crate) async fn delete_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    tracing::debug!(scope = ?scope, file_id = id, "soft deleting file");
    let file = get_info_in_scope(state, scope, id).await?;
    if file.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }
    file_repo::soft_delete(&state.db, id).await?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FileDeleted,
            scope,
            vec![file.id],
            vec![],
            vec![file.folder_id],
        ),
    );
    tracing::debug!(
        scope = ?scope,
        file_id = file.id,
        folder_id = file.folder_id,
        "soft deleted file"
    );
    Ok(())
}

/// 删除文件（软删除 → 回收站）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    delete_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub(crate) async fn cleanup_unreferenced_blob(state: &AppState, blob: &file_blob::Model) -> bool {
    async fn restore_cleanup_claim(state: &AppState, blob_id: i64, reason: &str) {
        match file_repo::restore_blob_cleanup_claim(&state.db, blob_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    blob_id,
                    "blob cleanup claim was already released while handling {reason}"
                );
            }
            Err(e) => {
                tracing::warn!(
                    blob_id,
                    "failed to restore blob cleanup claim after {reason}: {e}"
                );
            }
        }
    }

    let current_blob = match file_repo::find_blob_by_id(&state.db, blob.id).await {
        Ok(current_blob) => current_blob,
        Err(e) if e.code() == "E006" => return true,
        Err(e) => {
            tracing::warn!(
                blob_id = blob.id,
                "failed to reload blob before cleanup: {e}"
            );
            return false;
        }
    };

    if current_blob.ref_count != 0 {
        tracing::warn!(
            blob_id = current_blob.id,
            ref_count = current_blob.ref_count,
            "skipping blob cleanup because blob is referenced again"
        );
        return false;
    }

    match file_repo::claim_blob_cleanup(&state.db, current_blob.id).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "skipping blob cleanup because another worker already claimed it or it was revived"
            );
            return false;
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "failed to claim blob cleanup: {e}"
            );
            return false;
        }
    }

    if let Err(e) = thumbnail_service::delete_thumbnail(state, &current_blob).await {
        tracing::warn!(
            blob_id = current_blob.id,
            "failed to delete thumbnail during blob cleanup: {e}"
        );
    }

    let Some(policy) = state.policy_snapshot.get_policy(current_blob.policy_id) else {
        tracing::warn!(
            blob_id = current_blob.id,
            policy_id = current_blob.policy_id,
            "failed to load storage policy during blob cleanup: policy missing from snapshot"
        );
        restore_cleanup_claim(state, current_blob.id, "policy lookup failure").await;
        return false;
    };

    let driver = match state.driver_registry.get_driver(&policy) {
        Ok(driver) => driver,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                policy_id = current_blob.policy_id,
                "failed to resolve storage driver during blob cleanup: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "driver resolution failure").await;
            return false;
        }
    };

    let object_deleted = match driver.delete(&current_blob.storage_path).await {
        Ok(()) => true,
        Err(e) => match driver.exists(&current_blob.storage_path).await {
            Ok(false) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "blob delete returned error but object is already absent: {e}"
                );
                true
            }
            Ok(true) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object, keeping blob row for retry: {e}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete error").await;
                false
            }
            Err(exists_err) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object and verify existence, keeping blob row for retry: delete_error={e}, exists_error={exists_err}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete verification error").await;
                false
            }
        },
    };

    if !object_deleted {
        return false;
    }

    match file_repo::delete_blob_if_cleanup_claimed(&state.db, current_blob.id).await {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but cleanup claim was lost before deleting blob row"
            );
            restore_cleanup_claim(
                state,
                current_blob.id,
                "lost cleanup claim before row delete",
            )
            .await;
            false
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but failed to delete blob row: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "row delete failure").await;
            false
        }
    }
}

pub(crate) async fn purge_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let file = file_repo::find_by_id(&state.db, id).await?;
    workspace_storage_service::ensure_file_scope(&file, scope)?;

    batch_purge_in_scope(state, scope, vec![file]).await?;
    Ok(())
}

/// 永久删除文件，处理 blob ref_count、物理文件、缩略图和配额。
pub async fn purge(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

/// 批量永久删除文件：一次事务处理所有 DB 操作，事务后并行清理物理文件
///
/// 比逐个调 `purge()` 快得多——N 个文件只需 ~10 次 DB 查询而非 ~12N 次。
pub(crate) async fn batch_purge_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    files: Vec<file::Model>,
) -> Result<u32> {
    if files.is_empty() {
        return Ok(0);
    }

    let input_count = files.len();
    tracing::debug!(
        scope = ?scope,
        file_count = input_count,
        "purging files permanently"
    );

    for file in &files {
        workspace_storage_service::ensure_file_scope(file, scope)?;
    }

    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let blob_ids: Vec<i64> = files.iter().map(|f| f.blob_id).collect();
    let count = usize_to_u32(files.len(), "purged file count")?;

    // ── 单次事务：版本 → 属性 → 文件 → blob → 配额 ──
    let txn = crate::db::transaction::begin(&state.db).await?;

    // 1. 批量删除版本记录，收集版本 blob IDs
    let version_blob_ids =
        crate::db::repository::version_repo::delete_all_by_file_ids(&txn, &file_ids).await?;

    // 2. 批量删除文件属性
    crate::db::repository::property_repo::delete_all_for_entities(
        &txn,
        crate::types::EntityType::File,
        &file_ids,
    )
    .await?;

    // 3. 批量删除文件记录（先于 blob，解除 FK）
    file_repo::delete_many(&txn, &file_ids).await?;

    // 4. 处理 blob 引用计数
    //    合并主 blob 和版本 blob，按 blob_id 统计需要减少的引用数
    let mut blob_decrements: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &bid in &blob_ids {
        *blob_decrements.entry(bid).or_default() += 1;
    }
    for &vbid in &version_blob_ids {
        *blob_decrements.entry(vbid).or_default() += 1;
    }

    let blob_ids: Vec<i64> = blob_decrements.keys().copied().collect();
    let blobs_by_id = file_repo::find_blobs_by_ids(&txn, &blob_ids).await?;
    let mut blobs_to_cleanup: Vec<file_blob::Model> = Vec::new();
    let mut total_freed_bytes = 0i64;

    for (&blob_id, &decrement) in &blob_decrements {
        if let Some(blob) = blobs_by_id.get(&blob_id) {
            let freed_bytes = blob.size.checked_mul(decrement).ok_or_else(|| {
                AsterError::internal_error(format!(
                    "freed byte count overflow for blob {blob_id} during batch purge"
                ))
            })?;
            total_freed_bytes = total_freed_bytes.checked_add(freed_bytes).ok_or_else(|| {
                AsterError::internal_error("total freed byte count overflow during batch purge")
            })?;
            let decrement_i32 = i32::try_from(decrement).map_err(|_| {
                AsterError::internal_error(format!(
                    "blob decrement overflow for blob {blob_id} during batch purge"
                ))
            })?;
            file_repo::decrement_blob_ref_count_by(&txn, blob_id, decrement_i32).await?;
            if i64::from(blob.ref_count) <= decrement {
                blobs_to_cleanup.push(blob.clone());
            }
        }
    }

    // 5. 配额一次性更新
    workspace_storage_service::update_storage_used(&txn, scope, -total_freed_bytes).await?;

    crate::db::transaction::commit(txn).await?;

    // ── 事务后：并行物理清理，清理成功后再删 blob 元数据 ──
    stream::iter(blobs_to_cleanup)
        .for_each_concurrent(BLOB_CLEANUP_CONCURRENCY, |blob| async move {
            if !cleanup_unreferenced_blob(state, &blob).await {
                tracing::warn!(
                    blob_id = blob.id,
                    "batch purge left blob row for retry because object cleanup was incomplete"
                );
            }
        })
        .await;

    tracing::debug!(
        scope = ?scope,
        file_count = input_count,
        freed_bytes = total_freed_bytes,
        cleanup_blob_count = blob_ids.len(),
        "purged files permanently"
    );
    Ok(count)
}

pub async fn batch_purge(state: &AppState, files: Vec<file::Model>, user_id: i64) -> Result<u32> {
    batch_purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, files).await
}
