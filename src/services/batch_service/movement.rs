use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::TransactionTrait;

use crate::db::repository::{file_repo, folder_repo};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    storage_change_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

use super::shared::{load_folder_ancestor_ids_in_scope, load_target_folder_in_scope};
use super::{BatchResult, NormalizedSelection, load_normalized_selection_in_scope};

pub(crate) async fn batch_move_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    let mut result = BatchResult::new();
    let NormalizedSelection {
        file_ids: normalized_file_ids,
        folder_ids: normalized_folder_ids,
        file_map,
        folder_map,
    } = load_normalized_selection_in_scope(state, scope, file_ids, folder_ids).await?;

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

    for &id in &normalized_file_ids {
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

    for &id in &normalized_folder_ids {
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
        let file_parent_ids: Vec<Option<i64>> = file_ids_to_move
            .iter()
            .flat_map(|id| file_map.get(id).into_iter())
            .flat_map(|file| [file.folder_id, target_folder_id])
            .collect();
        let folder_parent_ids: Vec<Option<i64>> = folder_ids_to_move
            .iter()
            .flat_map(|id| folder_map.get(id).into_iter())
            .flat_map(|folder| [folder.parent_id, target_folder_id])
            .collect();

        let txn = state.db.begin().await.map_err(AsterError::from)?;
        file_repo::move_many_to_folder(&txn, &file_ids_to_move, target_folder_id, now).await?;
        folder_repo::move_many_to_parent(&txn, &folder_ids_to_move, target_folder_id, now).await?;
        txn.commit().await.map_err(AsterError::from)?;

        if !file_ids_to_move.is_empty() {
            storage_change_service::publish(
                state,
                storage_change_service::StorageChangeEvent::new(
                    storage_change_service::StorageChangeKind::FileUpdated,
                    scope,
                    file_ids_to_move,
                    vec![],
                    file_parent_ids,
                ),
            );
        }
        if !folder_ids_to_move.is_empty() {
            storage_change_service::publish(
                state,
                storage_change_service::StorageChangeEvent::new(
                    storage_change_service::StorageChangeKind::FolderUpdated,
                    scope,
                    vec![],
                    folder_ids_to_move,
                    folder_parent_ids,
                ),
            );
        }
    }

    Ok(result)
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
