use std::future::Future;
use std::pin::Pin;

use chrono::Utc;
use sea_orm::{Set, TransactionTrait};

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::folder;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::file_service;

/// 递归收集文件夹树内的所有文件和子文件夹 ID
///
/// - `include_deleted = true`：收集全部（含已软删除），用于 purge
/// - `include_deleted = false`：只收集未删除项，用于 soft_delete
pub async fn collect_folder_tree(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    folder_id: i64,
    include_deleted: bool,
) -> Result<(Vec<crate::entities::file::Model>, Vec<i64>)> {
    let mut files = Vec::new();
    let mut folder_ids = Vec::new();
    collect_tree_inner(
        db,
        user_id,
        folder_id,
        include_deleted,
        &mut files,
        &mut folder_ids,
    )
    .await?;
    Ok((files, folder_ids))
}

fn collect_tree_inner<'a>(
    db: &'a sea_orm::DatabaseConnection,
    user_id: i64,
    folder_id: i64,
    include_deleted: bool,
    files: &'a mut Vec<crate::entities::file::Model>,
    folder_ids: &'a mut Vec<i64>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        folder_ids.push(folder_id);

        let folder_files = if include_deleted {
            folder_repo::find_all_files_in_folder(db, folder_id).await?
        } else {
            file_repo::find_by_folder(db, user_id, Some(folder_id)).await?
        };
        files.extend(folder_files);

        let children = if include_deleted {
            folder_repo::find_all_children(db, folder_id).await?
        } else {
            folder_repo::find_children(db, user_id, Some(folder_id)).await?
        };
        for child in children {
            collect_tree_inner(db, user_id, child.id, include_deleted, files, folder_ids).await?;
        }

        Ok(())
    })
}

/// 递归软删除文件夹及其所有内容（→ 回收站）
///
/// 先收集所有未删除的文件和文件夹 ID，再一次事务内批量 soft_delete。
pub async fn recursive_soft_delete(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let (files, folder_ids) = collect_folder_tree(&state.db, user_id, folder_id, false).await?;

    let file_ids: Vec<i64> = files.into_iter().map(|f| f.id).collect();
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    file_repo::soft_delete_many(&txn, &file_ids, now).await?;
    folder_repo::soft_delete_many(&txn, &folder_ids, now).await?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(())
}

/// 递归永久删除文件夹及其所有内容（批量优化版）
///
/// 先递归收集所有文件和文件夹 ID（含已删除），然后一次 batch_purge 处理所有文件，
/// 再批量删除文件夹记录和属性。比逐个 purge 快得多。
pub async fn recursive_purge_folder(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let (all_files, all_folder_ids) =
        collect_folder_tree(&state.db, user_id, folder_id, true).await?;

    // ── 批量清理文件（一次事务 + 并行物理清理） ──
    if let Err(e) = file_service::batch_purge(state, all_files, user_id).await {
        tracing::warn!("batch purge files in folder #{folder_id} failed: {e}");
    }

    // ── 批量清理文件夹属性 ──
    crate::db::repository::property_repo::delete_all_for_entities(
        &state.db,
        crate::types::EntityType::Folder,
        &all_folder_ids,
    )
    .await?;

    // ── 批量硬删除文件夹记录 ──
    folder_repo::delete_many(&state.db, &all_folder_ids).await?;

    Ok(())
}

/// 递归复制文件夹及其所有内容到新位置
///
/// 利用 blob 去重：只增加 ref_count，不复制物理数据
pub fn recursive_copy_folder<'a>(
    state: &'a AppState,
    user_id: i64,
    src_folder_id: i64,
    dest_parent_id: Option<i64>,
    dest_name: &'a str,
) -> Pin<Box<dyn Future<Output = Result<folder::Model>> + Send + 'a>> {
    Box::pin(async move {
        let db = &state.db;
        let now = Utc::now();

        // 创建目标文件夹
        let new_folder = folder_repo::create(
            db,
            folder::ActiveModel {
                name: Set(dest_name.to_string()),
                parent_id: Set(dest_parent_id),
                user_id: Set(user_id),
                policy_id: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        // 复制文件：用 duplicate_file_record 统一处理
        let files = file_repo::find_by_folder(db, user_id, Some(src_folder_id)).await?;
        for f in &files {
            file_service::duplicate_file_record(state, f, Some(new_folder.id), &f.name).await?;
        }

        // 递归复制子文件夹
        let children = folder_repo::find_children(db, user_id, Some(src_folder_id)).await?;
        for child in children {
            recursive_copy_folder(state, user_id, child.id, Some(new_folder.id), &child.name)
                .await?;
        }

        Ok(new_folder)
    })
}
