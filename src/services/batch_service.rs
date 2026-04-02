use chrono::Utc;
use sea_orm::TransactionTrait;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
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

/// 单次批量操作最大条目数
pub const MAX_BATCH_ITEMS: usize = 1000;

/// 校验批量操作参数：至少一个 ID，不超过上限
pub fn validate_batch_ids(file_ids: &[i64], folder_ids: &[i64]) -> Result<()> {
    if file_ids.is_empty() && folder_ids.is_empty() {
        return Err(AsterError::validation_error(
            "at least one file or folder ID is required",
        ));
    }
    if file_ids.len() + folder_ids.len() > MAX_BATCH_ITEMS {
        return Err(AsterError::validation_error(format!(
            "batch size cannot exceed {MAX_BATCH_ITEMS} items",
        )));
    }
    Ok(())
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchResult {
    pub succeeded: u32,
    pub failed: u32,
    pub errors: Vec<BatchItemError>,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchItemError {
    pub entity_type: String,
    pub entity_id: i64,
    pub error: String,
}

impl BatchResult {
    fn new() -> Self {
        Self {
            succeeded: 0,
            failed: 0,
            errors: vec![],
        }
    }

    fn record_success(&mut self) {
        self.succeeded += 1;
    }

    fn record_failure(&mut self, entity_type: &str, entity_id: i64, error: String) {
        self.failed += 1;
        self.errors.push(BatchItemError {
            entity_type: entity_type.to_string(),
            entity_id,
            error,
        });
    }
}

fn build_file_map(files: Vec<file::Model>) -> HashMap<i64, file::Model> {
    files.into_iter().map(|file| (file.id, file)).collect()
}

fn build_folder_map(folders: Vec<folder::Model>) -> HashMap<i64, folder::Model> {
    folders
        .into_iter()
        .map(|folder| (folder.id, folder))
        .collect()
}

fn reserve_unique_name(reserved_names: &mut HashSet<String>, original_name: &str) -> String {
    let mut candidate = original_name.to_string();
    while !reserved_names.insert(candidate.clone()) {
        candidate = crate::utils::next_copy_name(&candidate);
    }
    candidate
}

async fn load_target_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    target_folder_id: Option<i64>,
) -> std::result::Result<Option<folder::Model>, String> {
    let Some(folder_id) = target_folder_id else {
        return Ok(None);
    };

    workspace_storage_service::verify_folder_access(state, scope, folder_id)
        .await
        .map(Some)
        .map_err(|e| e.to_string())
}

async fn load_folder_ancestor_ids_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    target_folder: Option<&folder::Model>,
) -> Result<HashSet<i64>> {
    let mut ancestors = HashSet::new();
    let mut current = target_folder.cloned();

    while let Some(folder) = current {
        workspace_storage_service::ensure_active_folder_scope(&folder, scope)?;
        ancestors.insert(folder.id);
        current = match folder.parent_id {
            Some(parent_id) => Some(folder_repo::find_by_id(&state.db, parent_id).await?),
            None => None,
        };
    }

    Ok(ancestors)
}

pub(crate) async fn batch_delete_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_ids: &[i64],
    folder_ids: &[i64],
) -> Result<BatchResult> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let mut result = BatchResult::new();
    let file_map = build_file_map(file_repo::find_by_ids(&state.db, file_ids).await?);
    let folder_map = build_folder_map(folder_repo::find_by_ids(&state.db, folder_ids).await?);

    let mut file_ids_to_delete = HashSet::new();
    let mut root_folder_ids_to_delete = Vec::new();
    let mut queued_root_folders = HashSet::new();

    for &id in file_ids {
        let Some(file) = file_map.get(&id) else {
            result.record_failure(
                "file",
                id,
                AsterError::file_not_found(format!("file #{id}")).to_string(),
            );
            continue;
        };
        if let Err(err) = workspace_storage_service::ensure_active_file_scope(file, scope) {
            result.record_failure("file", id, err.to_string());
            continue;
        }
        if file.is_locked {
            result.record_failure(
                "file",
                id,
                AsterError::resource_locked("file is locked").to_string(),
            );
            continue;
        }
        result.record_success();
        file_ids_to_delete.insert(id);
    }

    for &id in folder_ids {
        let Some(folder) = folder_map.get(&id) else {
            result.record_failure(
                "folder",
                id,
                AsterError::record_not_found(format!("folder #{id}")).to_string(),
            );
            continue;
        };
        if let Err(err) = workspace_storage_service::ensure_active_folder_scope(folder, scope) {
            result.record_failure("folder", id, err.to_string());
            continue;
        }
        if folder.is_locked {
            result.record_failure(
                "folder",
                id,
                AsterError::resource_locked("folder is locked").to_string(),
            );
            continue;
        }
        result.record_success();
        if queued_root_folders.insert(id) {
            root_folder_ids_to_delete.push(id);
        }
    }

    let mut folder_ids_to_delete = Vec::new();
    if !root_folder_ids_to_delete.is_empty() {
        let (tree_files, tree_folder_ids) = folder_service::collect_folder_forest_in_scope(
            &state.db,
            scope,
            &root_folder_ids_to_delete,
            false,
        )
        .await?;
        file_ids_to_delete.extend(tree_files.into_iter().map(|file| file.id));
        folder_ids_to_delete = tree_folder_ids;
    }

    if !file_ids_to_delete.is_empty() || !folder_ids_to_delete.is_empty() {
        let now = Utc::now();
        let file_ids_to_delete: Vec<i64> = file_ids_to_delete.into_iter().collect();

        let txn = state.db.begin().await.map_err(AsterError::from)?;
        file_repo::soft_delete_many(&txn, &file_ids_to_delete, now).await?;
        folder_repo::soft_delete_many(&txn, &folder_ids_to_delete, now).await?;
        txn.commit().await.map_err(AsterError::from)?;
    }

    Ok(result)
}

pub(crate) async fn batch_move_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let mut result = BatchResult::new();
    let file_map = build_file_map(file_repo::find_by_ids(&state.db, file_ids).await?);
    let folder_map = build_folder_map(folder_repo::find_by_ids(&state.db, folder_ids).await?);

    let (target_folder, target_error) =
        match load_target_folder_in_scope(state, scope, target_folder_id).await {
            Ok(folder) => (folder, None),
            Err(error) => (None, Some(error)),
        };

    let mut target_file_names = HashMap::new();
    let mut target_folder_names = HashMap::new();
    let mut target_ancestor_ids = HashSet::new();
    if target_error.is_none() {
        target_file_names =
            workspace_storage_service::list_files_in_folder(state, scope, target_folder_id)
                .await?
                .into_iter()
                .map(|file| (file.name, file.id))
                .collect();
        target_folder_names =
            workspace_storage_service::list_folders_in_parent(state, scope, target_folder_id)
                .await?
                .into_iter()
                .map(|folder| (folder.name, folder.id))
                .collect();
        target_ancestor_ids =
            load_folder_ancestor_ids_in_scope(state, scope, target_folder.as_ref()).await?;
    }

    let mut file_ids_to_move = HashSet::new();
    let mut folder_ids_to_move = HashSet::new();

    for &id in file_ids {
        let Some(file) = file_map.get(&id) else {
            result.record_failure(
                "file",
                id,
                AsterError::file_not_found(format!("file #{id}")).to_string(),
            );
            continue;
        };
        if let Err(err) = workspace_storage_service::ensure_active_file_scope(file, scope) {
            result.record_failure("file", id, err.to_string());
            continue;
        }
        if file.is_locked {
            result.record_failure(
                "file",
                id,
                AsterError::resource_locked("file is locked").to_string(),
            );
            continue;
        }
        if let Some(error) = target_error.as_ref() {
            result.record_failure("file", id, error.clone());
            continue;
        }
        if matches!(target_file_names.get(&file.name), Some(existing_id) if *existing_id != file.id)
        {
            result.record_failure(
                "file",
                id,
                AsterError::validation_error(format!(
                    "file '{}' already exists in target folder",
                    file.name
                ))
                .to_string(),
            );
            continue;
        }

        result.record_success();
        if file.folder_id != target_folder_id {
            file_ids_to_move.insert(file.id);
        }
        target_file_names.insert(file.name.clone(), file.id);
    }

    for &id in folder_ids {
        let Some(folder) = folder_map.get(&id) else {
            result.record_failure(
                "folder",
                id,
                AsterError::record_not_found(format!("folder #{id}")).to_string(),
            );
            continue;
        };
        if let Err(err) = workspace_storage_service::ensure_active_folder_scope(folder, scope) {
            result.record_failure("folder", id, err.to_string());
            continue;
        }
        if folder.is_locked {
            result.record_failure(
                "folder",
                id,
                AsterError::resource_locked("folder is locked").to_string(),
            );
            continue;
        }
        if target_folder_id == Some(folder.id) {
            result.record_failure(
                "folder",
                id,
                AsterError::validation_error("cannot move folder into itself").to_string(),
            );
            continue;
        }
        if let Some(error) = target_error.as_ref() {
            result.record_failure("folder", id, error.clone());
            continue;
        }
        if target_ancestor_ids.contains(&folder.id) {
            result.record_failure(
                "folder",
                id,
                AsterError::validation_error("cannot move folder into its own subfolder")
                    .to_string(),
            );
            continue;
        }
        if matches!(target_folder_names.get(&folder.name), Some(existing_id) if *existing_id != folder.id)
        {
            result.record_failure(
                "folder",
                id,
                AsterError::validation_error(format!(
                    "folder '{}' already exists in target folder",
                    folder.name
                ))
                .to_string(),
            );
            continue;
        }

        result.record_success();
        if folder.parent_id != target_folder_id {
            folder_ids_to_move.insert(folder.id);
        }
        target_folder_names.insert(folder.name.clone(), folder.id);
    }

    if !file_ids_to_move.is_empty() || !folder_ids_to_move.is_empty() {
        let now = Utc::now();
        let file_ids_to_move: Vec<i64> = file_ids_to_move.into_iter().collect();
        let folder_ids_to_move: Vec<i64> = folder_ids_to_move.into_iter().collect();

        let txn = state.db.begin().await.map_err(AsterError::from)?;
        file_repo::move_many_to_folder(&txn, &file_ids_to_move, target_folder_id, now).await?;
        folder_repo::move_many_to_parent(&txn, &folder_ids_to_move, target_folder_id, now).await?;
        txn.commit().await.map_err(AsterError::from)?;
    }

    Ok(result)
}

pub(crate) async fn batch_copy_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let mut result = BatchResult::new();
    let file_map = build_file_map(file_repo::find_by_ids(&state.db, file_ids).await?);
    let target_error = load_target_folder_in_scope(state, scope, target_folder_id)
        .await
        .err();

    let mut reserved_file_names: HashSet<String> = if target_error.is_none() {
        workspace_storage_service::list_files_in_folder(state, scope, target_folder_id)
            .await?
            .into_iter()
            .map(|file| file.name)
            .collect()
    } else {
        HashSet::new()
    };

    let (mut planned_storage_used, storage_quota) =
        workspace_storage_service::load_storage_limits(state, scope).await?;
    let mut file_copy_specs = Vec::new();

    for &id in file_ids {
        let Some(file) = file_map.get(&id) else {
            result.record_failure(
                "file",
                id,
                AsterError::file_not_found(format!("file #{id}")).to_string(),
            );
            continue;
        };
        if let Err(err) = workspace_storage_service::ensure_active_file_scope(file, scope) {
            result.record_failure("file", id, err.to_string());
            continue;
        }
        if let Some(error) = target_error.as_ref() {
            result.record_failure("file", id, error.clone());
            continue;
        }

        let projected_storage_used =
            planned_storage_used.checked_add(file.size).ok_or_else(|| {
                AsterError::internal_error("planned copied byte count overflow during batch copy")
            })?;
        if storage_quota > 0 && projected_storage_used > storage_quota {
            result.record_failure(
                "file",
                id,
                AsterError::storage_quota_exceeded(format!(
                    "quota {}, used {}, need {}",
                    storage_quota, planned_storage_used, file.size
                ))
                .to_string(),
            );
            continue;
        }

        let dest_name = reserve_unique_name(&mut reserved_file_names, &file.name);
        planned_storage_used = projected_storage_used;
        result.record_success();
        file_copy_specs.push(file_service::BatchDuplicateFileRecordSpec {
            src: file.clone(),
            dest_name,
        });
    }

    if !file_copy_specs.is_empty() {
        file_service::batch_duplicate_file_records_with_names_in_scope(
            state,
            scope,
            &file_copy_specs,
            target_folder_id,
        )
        .await?;
    }

    for &id in folder_ids {
        if let Some(error) = target_error.as_ref() {
            result.record_failure("folder", id, error.clone());
            continue;
        }

        match folder_service::copy_folder_in_scope(state, scope, id, target_folder_id).await {
            Ok(_) => result.record_success(),
            Err(e) => result.record_failure("folder", id, e.to_string()),
        }
    }

    Ok(result)
}

/// 批量删除（软删除 -> 回收站）
pub async fn batch_delete(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
) -> Result<BatchResult> {
    batch_delete_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_ids,
        folder_ids,
    )
    .await
}

/// 批量移动（target_folder_id = None 表示移到根目录）
pub async fn batch_move(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    batch_move_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_ids,
        folder_ids,
        target_folder_id,
    )
    .await
}

/// 批量复制（target_folder_id = None 表示复制到根目录）
///
/// 文件复制会先统一做权限/配额/命名预校验，再批量写入；
/// 文件夹复制仍复用高层递归 copy 流程以保持行为一致。
pub async fn batch_copy(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    batch_copy_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_ids,
        folder_ids,
        target_folder_id,
    )
    .await
}

/// 团队空间批量删除（软删除 -> 回收站）
pub async fn batch_delete_team(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
) -> Result<BatchResult> {
    batch_delete_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        file_ids,
        folder_ids,
    )
    .await
}

/// 团队空间批量移动（target_folder_id = None 表示移到团队根目录）
pub async fn batch_move_team(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    batch_move_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        file_ids,
        folder_ids,
        target_folder_id,
    )
    .await
}

/// 团队空间批量复制（target_folder_id = None 表示复制到团队根目录）
pub async fn batch_copy_team(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    batch_copy_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        file_ids,
        folder_ids,
        target_folder_id,
    )
    .await
}
