//! 回收站服务子模块：`purge`。

use crate::db::repository::{file_repo, folder_repo};
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::{
    file_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

use super::PURGE_ALL_BATCH_SIZE;
use super::common::{
    recursive_purge_folder_in_scope, verify_file_in_trash_in_scope, verify_folder_in_trash_in_scope,
};

/// 永久删除单个文件
pub async fn purge_file(state: &PrimaryAppState, id: i64, user_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Personal { user_id };
    tracing::debug!(scope = ?scope, file_id = id, "purging file from trash");
    let file = verify_file_in_trash_in_scope(state, scope, id).await?;
    file_service::batch_purge_in_scope(state, scope, vec![file]).await?;
    tracing::debug!(scope = ?scope, file_id = id, "purged file from trash");
    Ok(())
}

pub async fn purge_team_file(state: &PrimaryAppState, team_id: i64, id: i64, user_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    };
    tracing::debug!(scope = ?scope, file_id = id, "purging file from trash");
    let file = verify_file_in_trash_in_scope(state, scope, id).await?;
    file_service::batch_purge_in_scope(state, scope, vec![file]).await?;
    tracing::debug!(scope = ?scope, file_id = id, "purged file from trash");
    Ok(())
}

/// 永久删除单个文件夹（递归）
pub async fn purge_folder(state: &PrimaryAppState, id: i64, user_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Personal { user_id };
    tracing::debug!(scope = ?scope, folder_id = id, "purging folder from trash");
    verify_folder_in_trash_in_scope(state, scope, id).await?;
    recursive_purge_folder_in_scope(state, scope, id).await?;
    tracing::debug!(scope = ?scope, folder_id = id, "purged folder from trash");
    Ok(())
}

pub async fn purge_team_folder(
    state: &PrimaryAppState,
    team_id: i64,
    id: i64,
    user_id: i64,
) -> Result<()> {
    let scope = WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    };
    tracing::debug!(scope = ?scope, folder_id = id, "purging folder from trash");
    verify_folder_in_trash_in_scope(state, scope, id).await?;
    recursive_purge_folder_in_scope(state, scope, id).await?;
    tracing::debug!(scope = ?scope, folder_id = id, "purged folder from trash");
    Ok(())
}

async fn purge_all_in_scope(state: &PrimaryAppState, scope: WorkspaceStorageScope) -> Result<u32> {
    tracing::debug!(scope = ?scope, "purging all trash contents");
    workspace_storage_service::require_scope_access(state, scope).await?;
    let mut count: u32 = 0;

    let mut folder_cursor: Option<(chrono::DateTime<chrono::Utc>, i64)> = None;
    loop {
        let (top_folders, _) = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                folder_repo::find_top_level_deleted_cursor(
                    &state.db,
                    user_id,
                    PURGE_ALL_BATCH_SIZE,
                    folder_cursor,
                )
                .await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                folder_repo::find_top_level_deleted_by_team_cursor(
                    &state.db,
                    team_id,
                    PURGE_ALL_BATCH_SIZE,
                    folder_cursor,
                )
                .await?
            }
        };
        if top_folders.is_empty() {
            break;
        }

        folder_cursor = top_folders
            .last()
            .and_then(|folder| folder.deleted_at.map(|deleted_at| (deleted_at, folder.id)));
        for folder in top_folders {
            match recursive_purge_folder_in_scope(state, scope, folder.id).await {
                Ok(()) => count += 1,
                Err(error) => tracing::warn!("purge folder {} failed: {error}", folder.id),
            }
        }
    }

    let mut file_cursor = None;
    loop {
        let (top_files, _) = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                file_repo::find_top_level_deleted_paginated(
                    &state.db,
                    user_id,
                    PURGE_ALL_BATCH_SIZE,
                    file_cursor,
                )
                .await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                file_repo::find_top_level_deleted_by_team_paginated(
                    &state.db,
                    team_id,
                    PURGE_ALL_BATCH_SIZE,
                    file_cursor,
                )
                .await?
            }
        };
        if top_files.is_empty() {
            break;
        }

        file_cursor = top_files
            .last()
            .and_then(|file| file.deleted_at.map(|deleted_at| (deleted_at, file.id)));
        match file_service::batch_purge_in_scope(state, scope, top_files).await {
            Ok(purged) => count += purged,
            Err(error) => tracing::warn!("batch purge top-level files failed: {error}"),
        }
    }

    tracing::debug!(scope = ?scope, purged_count = count, "purged all trash contents");
    Ok(count)
}

/// 清空用户回收站（返回实际成功删除数量）
///
/// 只处理顶层已删除项（文件夹内子项由递归批量清理），
/// 避免同一文件被重复 purge。
pub async fn purge_all(state: &PrimaryAppState, user_id: i64) -> Result<u32> {
    purge_all_in_scope(state, WorkspaceStorageScope::Personal { user_id }).await
}

pub async fn purge_all_team(state: &PrimaryAppState, team_id: i64, user_id: i64) -> Result<u32> {
    purge_all_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
    )
    .await
}
