//! 回收站服务子模块：`common`。

use std::collections::{BTreeSet, HashMap};

use sea_orm::DatabaseConnection;

use crate::db::repository::{file_repo, folder_repo, property_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    file_service, folder_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::EntityType;

use super::models::{TrashFileItem, TrashFolderItem};

pub(super) async fn build_trash_path_cache(
    db: &DatabaseConnection,
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

pub(super) fn build_trash_file_item(
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

pub(super) fn build_trash_folder_item(
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

pub(super) fn parent_restore_target_unavailable(
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

pub(super) async fn verify_file_in_trash_in_scope(
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

pub(super) async fn verify_folder_in_trash_in_scope(
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

pub(super) async fn recursive_purge_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<()> {
    let (all_files, all_folder_ids) =
        folder_service::collect_folder_tree_in_scope(&state.db, scope, folder_id, true).await?;
    file_service::batch_purge_in_scope(state, scope, all_files).await?;
    property_repo::delete_all_for_entities(&state.db, EntityType::Folder, &all_folder_ids).await?;
    folder_repo::delete_many(&state.db, &all_folder_ids).await?;
    Ok(())
}
