//! 服务模块：`webdav_service`。

use std::future::Future;
use std::pin::Pin;

use chrono::Utc;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::folder;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    file_service, folder_service, storage_change_service, workspace_models::FileInfo,
    workspace_storage_service::WorkspaceStorageScope,
};

/// 递归收集文件夹树内的所有文件和子文件夹 ID
///
/// - `include_deleted = true`：收集全部（含已软删除），用于 purge
/// - `include_deleted = false`：只收集未删除项，用于 soft_delete
async fn collect_folder_tree_models(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    folder_id: i64,
    include_deleted: bool,
) -> Result<(Vec<crate::entities::file::Model>, Vec<i64>)> {
    folder_service::collect_folder_tree_in_scope(
        db,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        include_deleted,
    )
    .await
}

pub async fn collect_folder_tree(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    folder_id: i64,
    include_deleted: bool,
) -> Result<(Vec<FileInfo>, Vec<i64>)> {
    collect_folder_tree_models(db, user_id, folder_id, include_deleted)
        .await
        .map(|(files, folder_ids)| (files.into_iter().map(FileInfo::from).collect(), folder_ids))
}

/// 递归软删除文件夹及其所有内容（→ 回收站）
///
/// 先收集所有未删除的文件和文件夹 ID，再一次事务内批量 soft_delete。
pub async fn recursive_soft_delete(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Personal { user_id };
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    let (files, folder_ids) =
        collect_folder_tree_models(&state.db, user_id, folder_id, false).await?;

    let file_ids: Vec<i64> = files.into_iter().map(|f| f.id).collect();
    let now = Utc::now();

    let txn = crate::db::transaction::begin(&state.db).await?;
    file_repo::soft_delete_many(&txn, &file_ids, now).await?;
    folder_repo::soft_delete_many(&txn, &folder_ids, now).await?;
    crate::db::transaction::commit(txn).await?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FolderDeleted,
            scope,
            vec![],
            vec![folder.id],
            vec![folder.parent_id],
        ),
    );

    Ok(())
}

/// 递归永久删除文件夹及其所有内容（批量优化版）
///
/// 先递归收集所有文件和文件夹 ID（含已删除），然后一次 batch_purge 处理所有文件，
/// 再批量删除文件夹记录和属性。比逐个 purge 快得多。
pub async fn recursive_purge_folder(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let (all_files, all_folder_ids) =
        collect_folder_tree_models(&state.db, user_id, folder_id, true).await?;

    file_service::batch_purge_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        all_files,
    )
    .await?;

    crate::db::repository::property_repo::delete_all_for_entities(
        &state.db,
        crate::types::EntityType::Folder,
        &all_folder_ids,
    )
    .await?;

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
        let scope =
            crate::services::workspace_storage_service::WorkspaceStorageScope::Personal { user_id };
        let copied = crate::services::folder_service::recursive_copy_folder_in_scope(
            state,
            scope,
            src_folder_id,
            dest_parent_id,
            dest_name,
        )
        .await?;
        storage_change_service::publish(
            state,
            storage_change_service::StorageChangeEvent::new(
                storage_change_service::StorageChangeKind::FolderCreated,
                scope,
                vec![],
                vec![copied.id],
                vec![copied.parent_id],
            ),
        );
        Ok(copied)
    })
}
