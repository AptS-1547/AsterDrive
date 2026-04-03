use std::collections::{BTreeSet, HashMap};

use sea_orm::TransactionTrait;
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    file_service, folder_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

const DEFAULT_RETENTION_DAYS: i64 = 7;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TrashFileItem {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub mime_type: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub original_path: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TrashFolderItem {
    pub id: i64,
    pub name: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub original_path: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TrashContents {
    pub folders: Vec<TrashFolderItem>,
    pub files: Vec<TrashFileItem>,
    pub folders_total: u64,
    pub files_total: u64,
    /// 下一页 cursor，None 表示已到最后一页
    pub next_file_cursor: Option<TrashFileCursor>,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TrashFileCursor {
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub id: i64,
}

async fn list_trash_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(chrono::DateTime<chrono::Utc>, i64)>,
) -> Result<TrashContents> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let (raw_folders, folders_total) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_top_level_deleted_paginated(
                &state.db,
                user_id,
                folder_limit,
                folder_offset,
            )
            .await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_top_level_deleted_by_team_paginated(
                &state.db,
                team_id,
                folder_limit,
                folder_offset,
            )
            .await?
        }
    };

    let (raw_files, files_total) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_top_level_deleted_paginated(&state.db, user_id, file_limit, file_cursor)
                .await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_top_level_deleted_by_team_paginated(
                &state.db,
                team_id,
                file_limit,
                file_cursor,
            )
            .await?
        }
    };

    let folder_paths = build_trash_path_cache(&state.db, &raw_folders, &raw_files).await?;

    let next_file_cursor = if file_limit > 0 && raw_files.len() as u64 == file_limit {
        raw_files.last().and_then(|f| {
            f.deleted_at.map(|ts| TrashFileCursor {
                deleted_at: ts,
                id: f.id,
            })
        })
    } else {
        None
    };

    let folders = raw_folders
        .into_iter()
        .map(|folder| build_trash_folder_item(folder, &folder_paths))
        .collect::<Result<Vec<_>>>()?;

    let files = raw_files
        .into_iter()
        .map(|file| build_trash_file_item(file, &folder_paths))
        .collect::<Result<Vec<_>>>()?;

    Ok(TrashContents {
        folders,
        files,
        folders_total,
        files_total,
        next_file_cursor,
    })
}

/// 列出用户回收站内容（分页）
pub async fn list_trash(
    state: &AppState,
    user_id: i64,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(chrono::DateTime<chrono::Utc>, i64)>,
) -> Result<TrashContents> {
    list_trash_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
    )
    .await
}

pub async fn list_team_trash(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(chrono::DateTime<chrono::Utc>, i64)>,
) -> Result<TrashContents> {
    list_trash_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
    )
    .await
}

async fn build_trash_path_cache(
    db: &sea_orm::DatabaseConnection,
    folders: &[folder::Model],
    files: &[file::Model],
) -> Result<HashMap<i64, String>> {
    let folder_ids: Vec<i64> = folders
        .iter()
        .filter_map(|folder| folder.parent_id)
        .chain(files.iter().filter_map(|file| file.folder_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    folder_service::build_folder_paths(db, &folder_ids).await
}

fn build_trash_file_item(
    file: file::Model,
    folder_paths: &HashMap<i64, String>,
) -> Result<TrashFileItem> {
    Ok(TrashFileItem {
        id: file.id,
        name: file.name,
        size: file.size,
        mime_type: file.mime_type,
        created_at: file.created_at,
        updated_at: file.updated_at,
        deleted_at: file
            .deleted_at
            .ok_or_else(|| AsterError::validation_error("file is not in trash"))?,
        is_locked: file.is_locked,
        original_path: resolve_folder_path(folder_paths, file.folder_id)?,
    })
}

fn build_trash_folder_item(
    folder: folder::Model,
    folder_paths: &HashMap<i64, String>,
) -> Result<TrashFolderItem> {
    Ok(TrashFolderItem {
        id: folder.id,
        name: folder.name,
        created_at: folder.created_at,
        updated_at: folder.updated_at,
        deleted_at: folder
            .deleted_at
            .ok_or_else(|| AsterError::validation_error("folder is not in trash"))?,
        is_locked: folder.is_locked,
        original_path: resolve_folder_path(folder_paths, folder.parent_id)?,
    })
}

fn resolve_folder_path(
    folder_paths: &HashMap<i64, String>,
    folder_id: Option<i64>,
) -> Result<String> {
    match folder_id {
        Some(folder_id) => folder_paths
            .get(&folder_id)
            .cloned()
            .ok_or_else(|| AsterError::record_not_found(format!("folder #{folder_id}"))),
        None => Ok("/".to_string()),
    }
}

fn parent_restore_target_unavailable(
    parent_result: &Result<folder::Model>,
    scope: WorkspaceStorageScope,
) -> Result<bool> {
    match parent_result {
        Ok(parent) => match workspace_storage_service::ensure_folder_scope(parent, scope) {
            Ok(()) => Ok(parent.deleted_at.is_some()),
            Err(AsterError::AuthForbidden(_))
            | Err(AsterError::RecordNotFound(_))
            | Err(AsterError::FileNotFound(_))
            | Err(AsterError::FolderNotFound(_)) => Ok(true),
            Err(error) => Err(error),
        },
        Err(AsterError::AuthForbidden(_))
        | Err(AsterError::RecordNotFound(_))
        | Err(AsterError::FileNotFound(_))
        | Err(AsterError::FolderNotFound(_)) => Ok(true),
        Err(error) => Err(error.clone()),
    }
}

async fn verify_file_in_trash_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<file::Model> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    workspace_storage_service::ensure_file_scope(&file, scope)?;
    if file.deleted_at.is_none() {
        return Err(AsterError::validation_error("file is not in trash"));
    }
    Ok(file)
}

async fn verify_folder_in_trash_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<folder::Model> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    workspace_storage_service::ensure_folder_scope(&folder, scope)?;
    if folder.deleted_at.is_none() {
        return Err(AsterError::validation_error("folder is not in trash"));
    }
    Ok(folder)
}

async fn recursive_restore_deleted_tree_in_scope(
    db: &sea_orm::DatabaseConnection,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<()> {
    let (files, folder_ids) =
        folder_service::collect_folder_tree_in_scope(db, scope, folder_id, true).await?;
    let child_folder_ids: Vec<i64> = folder_ids
        .into_iter()
        .filter(|&id| id != folder_id)
        .collect();
    let file_ids: Vec<i64> = files.into_iter().map(|file| file.id).collect();

    let txn = db.begin().await.map_err(AsterError::from)?;
    file_repo::restore_many(&txn, &file_ids).await?;
    folder_repo::restore_many(&txn, &child_folder_ids).await?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(())
}

async fn recursive_purge_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<()> {
    let (all_files, all_folder_ids) =
        folder_service::collect_folder_tree_in_scope(&state.db, scope, folder_id, true).await?;
    file_service::batch_purge_in_scope(state, scope, all_files).await?;

    crate::db::repository::property_repo::delete_all_for_entities(
        &state.db,
        crate::types::EntityType::Folder,
        &all_folder_ids,
    )
    .await?;

    folder_repo::delete_many(&state.db, &all_folder_ids).await?;
    Ok(())
}

async fn restore_file_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    let file = verify_file_in_trash_in_scope(state, scope, id).await?;

    if let Some(folder_id) = file.folder_id {
        let parent = folder_repo::find_by_id(&state.db, folder_id).await;
        if parent_restore_target_unavailable(&parent, scope)? {
            let mut active: file::ActiveModel = file.into();
            active.folder_id = sea_orm::Set(None);
            active.deleted_at = sea_orm::Set(None);
            use sea_orm::ActiveModelTrait;
            active.update(&state.db).await.map_err(AsterError::from)?;
            return Ok(());
        }
    }

    file_repo::restore(&state.db, id).await
}

async fn restore_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    let folder = verify_folder_in_trash_in_scope(state, scope, id).await?;

    if let Some(parent_id) = folder.parent_id {
        let parent = folder_repo::find_by_id(&state.db, parent_id).await;
        if parent_restore_target_unavailable(&parent, scope)? {
            let mut active: folder::ActiveModel = folder.into();
            active.parent_id = sea_orm::Set(None);
            active.deleted_at = sea_orm::Set(None);
            use sea_orm::ActiveModelTrait;
            active.update(&state.db).await.map_err(AsterError::from)?;
            recursive_restore_deleted_tree_in_scope(&state.db, scope, id).await?;
            return Ok(());
        }
    }

    folder_repo::restore(&state.db, id).await?;
    recursive_restore_deleted_tree_in_scope(&state.db, scope, id).await
}

/// 恢复文件
pub async fn restore_file(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    restore_file_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub async fn restore_team_file(
    state: &AppState,
    team_id: i64,
    id: i64,
    user_id: i64,
) -> Result<()> {
    restore_file_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        id,
    )
    .await
}

/// 恢复文件夹（递归恢复子项）
pub async fn restore_folder(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    restore_folder_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub async fn restore_team_folder(
    state: &AppState,
    team_id: i64,
    id: i64,
    user_id: i64,
) -> Result<()> {
    restore_folder_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        id,
    )
    .await
}

/// 永久删除单个文件
pub async fn purge_file(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Personal { user_id };
    let file = verify_file_in_trash_in_scope(state, scope, id).await?;
    file_service::batch_purge_in_scope(state, scope, vec![file]).await?;
    Ok(())
}

pub async fn purge_team_file(state: &AppState, team_id: i64, id: i64, user_id: i64) -> Result<()> {
    let scope = WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    };
    let file = verify_file_in_trash_in_scope(state, scope, id).await?;
    file_service::batch_purge_in_scope(state, scope, vec![file]).await?;
    Ok(())
}

/// 永久删除单个文件夹（递归）
pub async fn purge_folder(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    verify_folder_in_trash_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await?;
    recursive_purge_folder_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub async fn purge_team_folder(
    state: &AppState,
    team_id: i64,
    id: i64,
    user_id: i64,
) -> Result<()> {
    let scope = WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    };
    verify_folder_in_trash_in_scope(state, scope, id).await?;
    recursive_purge_folder_in_scope(state, scope, id).await
}

async fn purge_all_in_scope(state: &AppState, scope: WorkspaceStorageScope) -> Result<u32> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let mut count: u32 = 0;

    let (top_folders, _) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_top_level_deleted_paginated(&state.db, user_id, 10000, 0).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_top_level_deleted_by_team_paginated(&state.db, team_id, 10000, 0)
                .await?
        }
    };
    for folder in top_folders {
        match recursive_purge_folder_in_scope(state, scope, folder.id).await {
            Ok(()) => count += 1,
            Err(e) => tracing::warn!("purge folder {} failed: {e}", folder.id),
        }
    }

    let (top_files, _) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_top_level_deleted_paginated(&state.db, user_id, 10000, None).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_top_level_deleted_by_team_paginated(&state.db, team_id, 10000, None)
                .await?
        }
    };
    if !top_files.is_empty() {
        let file_count = top_files.len() as u32;
        match file_service::batch_purge_in_scope(state, scope, top_files).await {
            Ok(_) => count += file_count,
            Err(e) => tracing::warn!("batch purge top-level files failed: {e}"),
        }
    }

    Ok(count)
}

/// 清空用户回收站（返回实际成功删除数量）
///
/// 只处理顶层已删除项（文件夹内子项由递归批量清理），
/// 避免同一文件被重复 purge。
pub async fn purge_all(state: &AppState, user_id: i64) -> Result<u32> {
    purge_all_in_scope(state, WorkspaceStorageScope::Personal { user_id }).await
}

pub async fn purge_all_team(state: &AppState, team_id: i64, user_id: i64) -> Result<u32> {
    purge_all_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
    )
    .await
}

/// 自动清理过期回收站条目（后台任务调用）
pub async fn cleanup_expired(state: &AppState) -> Result<u32> {
    let retention_days = state
        .runtime_config
        .get_i64("trash_retention_days")
        .unwrap_or_else(|| {
            if let Some(raw) = state.runtime_config.get("trash_retention_days") {
                tracing::warn!(
                    "invalid trash_retention_days value '{}', using default",
                    raw
                );
            }
            DEFAULT_RETENTION_DAYS
        });

    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
    let mut count: u32 = 0;

    // 清理过期文件（批量）
    let expired_files = file_repo::find_expired_deleted(&state.db, cutoff).await?;
    let expired_file_count = expired_files.len() as u32;
    let mut by_user: std::collections::HashMap<i64, Vec<file::Model>> =
        std::collections::HashMap::new();
    let mut by_team: std::collections::HashMap<i64, Vec<file::Model>> =
        std::collections::HashMap::new();
    for file in expired_files {
        if let Some(team_id) = file.team_id {
            by_team.entry(team_id).or_default().push(file);
        } else {
            by_user.entry(file.user_id).or_default().push(file);
        }
    }
    for (uid, files) in by_user {
        if let Err(e) = file_service::batch_purge_in_scope(
            state,
            WorkspaceStorageScope::Personal { user_id: uid },
            files,
        )
        .await
        {
            tracing::warn!("trash cleanup expired files for user #{uid} failed: {e}");
        }
    }
    for (team_id, files) in by_team {
        if let Err(e) = file_service::batch_purge_in_scope(
            state,
            WorkspaceStorageScope::Team {
                team_id,
                actor_user_id: 0,
            },
            files,
        )
        .await
        {
            tracing::warn!("trash cleanup expired files for team #{team_id} failed: {e}");
        }
    }
    count += expired_file_count;

    // 清理过期文件夹——只处理顶层（父文件夹也过期则由父递归处理，避免重复）
    let expired_folders = folder_repo::find_expired_deleted(&state.db, cutoff).await?;
    let expired_folder_ids: std::collections::HashSet<i64> =
        expired_folders.iter().map(|f| f.id).collect();
    let top_level_folders: Vec<&folder::Model> = expired_folders
        .iter()
        .filter(|f| {
            f.parent_id
                .is_none_or(|pid| !expired_folder_ids.contains(&pid))
        })
        .collect();
    for f in &top_level_folders {
        let result = if let Some(team_id) = f.team_id {
            recursive_purge_folder_in_scope(
                state,
                WorkspaceStorageScope::Team {
                    team_id,
                    actor_user_id: 0,
                },
                f.id,
            )
            .await
        } else {
            recursive_purge_folder_in_scope(
                state,
                WorkspaceStorageScope::Personal { user_id: f.user_id },
                f.id,
            )
            .await
        };
        if let Err(e) = result {
            tracing::warn!("trash cleanup folder {} failed: {e}", f.id);
        }
    }
    count += top_level_folders.len() as u32;

    if count > 0 {
        tracing::info!("trash cleanup: purged {count} expired items (retention={retention_days}d)");
    }
    Ok(count)
}
