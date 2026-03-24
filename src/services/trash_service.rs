use std::collections::HashSet;

use serde::Serialize;
use utoipa::ToSchema;

use crate::db::repository::{config_repo, file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{file_service, webdav_service};

const DEFAULT_RETENTION_DAYS: i64 = 7;

#[derive(Serialize, ToSchema)]
pub struct TrashFileItem {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub mime_type: String,
    #[schema(value_type = String)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub original_path: String,
}

#[derive(Serialize, ToSchema)]
pub struct TrashFolderItem {
    pub id: i64,
    pub name: String,
    #[schema(value_type = String)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub original_path: String,
}

#[derive(Serialize, ToSchema)]
pub struct TrashContents {
    pub folders: Vec<TrashFolderItem>,
    pub files: Vec<TrashFileItem>,
}

/// 列出用户回收站内容
pub async fn list_trash(state: &AppState, user_id: i64) -> Result<TrashContents> {
    let deleted_folders = folder_repo::find_deleted_by_user(&state.db, user_id).await?;
    let deleted_folder_ids = deleted_folders
        .iter()
        .map(|folder| folder.id)
        .collect::<HashSet<_>>();

    let mut folders = Vec::new();
    for folder in deleted_folders {
        if is_top_level_deleted_folder(&deleted_folder_ids, &folder) {
            folders.push(build_trash_folder_item(&state.db, folder).await?);
        }
    }

    let mut files = Vec::new();
    for file in file_repo::find_deleted_by_user(&state.db, user_id).await? {
        if is_top_level_deleted_file(&deleted_folder_ids, &file) {
            files.push(build_trash_file_item(&state.db, file).await?);
        }
    }

    Ok(TrashContents { folders, files })
}

fn is_top_level_deleted_folder(deleted_folder_ids: &HashSet<i64>, folder: &folder::Model) -> bool {
    match folder.parent_id {
        Some(parent_id) => !deleted_folder_ids.contains(&parent_id),
        None => true,
    }
}

fn is_top_level_deleted_file(deleted_folder_ids: &HashSet<i64>, file: &file::Model) -> bool {
    match file.folder_id {
        Some(folder_id) => !deleted_folder_ids.contains(&folder_id),
        None => true,
    }
}

async fn build_trash_file_item(
    db: &sea_orm::DatabaseConnection,
    file: file::Model,
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
        original_path: resolve_folder_path(db, file.folder_id).await?,
    })
}

async fn build_trash_folder_item(
    db: &sea_orm::DatabaseConnection,
    folder: folder::Model,
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
        original_path: resolve_folder_path(db, folder.parent_id).await?,
    })
}

async fn resolve_folder_path(
    db: &sea_orm::DatabaseConnection,
    folder_id: Option<i64>,
) -> Result<String> {
    let mut segments = Vec::new();
    let mut current = folder_id;

    while let Some(folder_id) = current {
        let folder = folder_repo::find_by_id(db, folder_id).await?;
        segments.push(folder.name);
        current = folder.parent_id;
    }

    segments.reverse();
    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

/// 恢复文件
pub async fn restore_file(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.deleted_at.is_none() {
        return Err(AsterError::validation_error("file is not in trash"));
    }

    // 如果原文件夹已删除，恢复到根目录
    if let Some(fid) = f.folder_id {
        let folder = folder_repo::find_by_id(&state.db, fid).await;
        if folder.is_err() || folder.is_ok_and(|f| f.deleted_at.is_some()) {
            // 原文件夹不存在或已删除，移到根目录
            let mut active: file::ActiveModel = f.into();
            active.folder_id = sea_orm::Set(None);
            active.deleted_at = sea_orm::Set(None);
            use sea_orm::ActiveModelTrait;
            active.update(&state.db).await.map_err(AsterError::from)?;
            return Ok(());
        }
    }

    file_repo::restore(&state.db, id).await
}

/// 恢复文件夹（递归恢复子项）
pub async fn restore_folder(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = folder_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;
    if f.deleted_at.is_none() {
        return Err(AsterError::validation_error("folder is not in trash"));
    }

    // 如果父文件夹已删除，恢复到根目录
    if let Some(pid) = f.parent_id {
        let parent = folder_repo::find_by_id(&state.db, pid).await;
        if parent.is_err() || parent.is_ok_and(|p| p.deleted_at.is_some()) {
            let mut active: folder::ActiveModel = f.into();
            active.parent_id = sea_orm::Set(None);
            active.deleted_at = sea_orm::Set(None);
            use sea_orm::ActiveModelTrait;
            active.update(&state.db).await.map_err(AsterError::from)?;
            // 还需要恢复子项
            recursive_restore(&state.db, user_id, id).await?;
            return Ok(());
        }
    }

    folder_repo::restore(&state.db, id).await?;
    recursive_restore(&state.db, user_id, id).await
}

/// 递归恢复子文件和子文件夹
async fn recursive_restore(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    folder_id: i64,
) -> Result<()> {
    // 恢复该文件夹下的已删除文件（精确查询，不查全量）
    let deleted_files = file_repo::find_deleted_in_folder(db, folder_id).await?;
    for f in deleted_files {
        file_repo::restore(db, f.id).await?;
    }

    // 恢复已删除的子文件夹
    let deleted_folders = folder_repo::find_deleted_children(db, folder_id).await?;
    for child in deleted_folders {
        folder_repo::restore(db, child.id).await?;
        Box::pin(recursive_restore(db, user_id, child.id)).await?;
    }

    Ok(())
}

/// 永久删除单个文件
pub async fn purge_file(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.deleted_at.is_none() {
        return Err(AsterError::validation_error("file is not in trash"));
    }
    file_service::purge(state, id, user_id).await
}

/// 永久删除单个文件夹（递归）
pub async fn purge_folder(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = folder_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;
    if f.deleted_at.is_none() {
        return Err(AsterError::validation_error("folder is not in trash"));
    }
    webdav_service::recursive_purge_folder(state, user_id, id).await
}

/// 清空用户回收站（返回实际成功删除数量）
pub async fn purge_all(state: &AppState, user_id: i64) -> Result<u32> {
    let files = file_repo::find_deleted_by_user(&state.db, user_id).await?;
    let folders = folder_repo::find_deleted_by_user(&state.db, user_id).await?;
    let mut count: u32 = 0;

    for f in files {
        match file_service::purge(state, f.id, user_id).await {
            Ok(()) => count += 1,
            Err(e) => tracing::warn!("purge file {} failed: {e}", f.id),
        }
    }
    for f in folders {
        match webdav_service::recursive_purge_folder(state, user_id, f.id).await {
            Ok(()) => count += 1,
            Err(e) => tracing::warn!("purge folder {} failed: {e}", f.id),
        }
    }

    Ok(count)
}

/// 自动清理过期回收站条目（后台任务调用）
pub async fn cleanup_expired(state: &AppState) -> Result<u32> {
    let retention_days = match config_repo::find_by_key(&state.db, "trash_retention_days").await? {
        Some(cfg) => cfg.value.parse::<i64>().unwrap_or_else(|_| {
            tracing::warn!(
                "invalid trash_retention_days value '{}', using default",
                cfg.value
            );
            DEFAULT_RETENTION_DAYS
        }),
        None => DEFAULT_RETENTION_DAYS,
    };

    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
    let mut count: u32 = 0;

    // 清理过期文件
    let expired_files = file_repo::find_expired_deleted(&state.db, cutoff).await?;
    for f in &expired_files {
        if let Err(e) = file_service::purge(state, f.id, f.user_id).await {
            tracing::warn!("trash cleanup file {} failed: {e}", f.id);
        }
    }
    count += expired_files.len() as u32;

    // 清理过期文件夹
    let expired_folders = folder_repo::find_expired_deleted(&state.db, cutoff).await?;
    for f in &expired_folders {
        if let Err(e) = webdav_service::recursive_purge_folder(state, f.user_id, f.id).await {
            tracing::warn!("trash cleanup folder {} failed: {e}", f.id);
        }
    }
    count += expired_folders.len() as u32;

    if count > 0 {
        tracing::info!("trash cleanup: purged {count} expired items (retention={retention_days}d)");
    }
    Ok(count)
}
