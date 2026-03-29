use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};

use crate::db::repository::{config_repo, file_repo, version_repo};
use crate::entities::file_version;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

/// 列出文件的所有版本
pub async fn list_versions(
    state: &AppState,
    file_id: i64,
    user_id: i64,
) -> Result<Vec<file_version::Model>> {
    let f = file_repo::find_by_id(&state.db, file_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    version_repo::find_by_file_id(&state.db, file_id).await
}

/// 恢复到指定版本，并截断该版本及之后的历史版本
pub async fn restore_version(
    state: &AppState,
    file_id: i64,
    version_id: i64,
    user_id: i64,
) -> Result<crate::entities::file::Model> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, file_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }

    let version = version_repo::find_by_id(db, version_id)
        .await?
        .ok_or_else(|| AsterError::record_not_found("version not found"))?;

    if version.file_id != file_id {
        return Err(AsterError::record_not_found("version not found"));
    }

    let now = Utc::now();

    // 删除当前 blob 的缩略图（恢复后缩略图按需重新生成）
    let current_blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    if let Err(e) = crate::services::thumbnail_service::delete_thumbnail(state, &current_blob).await
    {
        tracing::warn!(
            "failed to delete thumbnail for blob {}: {e}",
            current_blob.id
        );
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;

    let previous_blob_id = current_blob.id;
    let target_blob_id = version.blob_id;

    let mut active: crate::entities::file::ActiveModel = f.into();
    active.blob_id = Set(target_blob_id);
    active.updated_at = Set(now);
    let updated = active.update(&txn).await.map_err(AsterError::from)?;

    let truncated_blob_ids =
        version_repo::delete_by_file_id_from_version(&txn, file_id, version.version).await?;

    txn.commit().await.map_err(AsterError::from)?;

    let mut cleanup_counts = std::collections::HashMap::<i64, usize>::new();
    for blob_id in truncated_blob_ids {
        *cleanup_counts.entry(blob_id).or_default() += 1;
    }

    if previous_blob_id != target_blob_id {
        *cleanup_counts.entry(previous_blob_id).or_default() += 1;
        if let Some(count) = cleanup_counts.get_mut(&target_blob_id) {
            *count = count.saturating_sub(1);
        }
    }

    for (blob_id, count) in cleanup_counts {
        for _ in 0..count {
            cleanup_blob_if_unused(state, blob_id).await?;
        }
    }

    Ok(updated)
}

/// 删除指定版本（减 blob ref_count）
pub async fn delete_version(
    state: &AppState,
    file_id: i64,
    version_id: i64,
    user_id: i64,
) -> Result<()> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, file_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;

    let version = version_repo::find_by_id(db, version_id)
        .await?
        .ok_or_else(|| AsterError::record_not_found("version not found"))?;

    if version.file_id != file_id {
        return Err(AsterError::record_not_found("version not found"));
    }

    version_repo::delete_by_id(db, version_id).await?;
    cleanup_blob_if_unused(state, version.blob_id).await?;

    Ok(())
}

/// 超出版本上限时清理最旧版本
pub async fn cleanup_excess(state: &AppState, file_id: i64) -> Result<()> {
    let db = &state.db;
    let max_versions = get_max_versions(state).await;

    loop {
        let count = version_repo::count_by_file_id(db, file_id).await?;
        if count <= max_versions {
            break;
        }
        let oldest = version_repo::find_oldest_by_file_id(db, file_id).await?;
        if let Some(oldest) = oldest {
            version_repo::delete_by_id(db, oldest.id).await?;
            cleanup_blob_if_unused(state, oldest.blob_id).await?;
        } else {
            break;
        }
    }

    Ok(())
}

/// 清理所有版本（文件永久删除时调用）
pub async fn purge_all_versions(state: &AppState, file_id: i64) -> Result<()> {
    let db = &state.db;
    let blob_ids = version_repo::delete_all_by_file_id(db, file_id).await?;

    for blob_id in blob_ids {
        cleanup_blob_if_unused(state, blob_id).await?;
    }

    Ok(())
}

/// 如果 blob 不再被任何文件或版本引用，减 ref_count 并可能删除物理文件
async fn cleanup_blob_if_unused(state: &AppState, blob_id: i64) -> Result<()> {
    let db = &state.db;
    let blob = file_repo::find_blob_by_id(db, blob_id).await?;

    if blob.ref_count <= 1 {
        file_repo::decrement_blob_ref_count(db, blob.id).await?;
        if !crate::services::file_service::cleanup_unreferenced_blob(state, &blob).await {
            tracing::warn!(
                blob_id = blob.id,
                "blob cleanup incomplete after version cleanup; blob row retained for retry"
            );
        }
    } else {
        file_repo::decrement_blob_ref_count(db, blob.id).await?;
    }

    Ok(())
}

async fn get_max_versions(state: &AppState) -> u64 {
    match config_repo::find_by_key(&state.db, "max_versions_per_file").await {
        Ok(Some(cfg)) => cfg.value.parse().unwrap_or_else(|_| {
            tracing::warn!(
                "invalid max_versions_per_file value '{}', using 10",
                cfg.value
            );
            10
        }),
        _ => 10,
    }
}
