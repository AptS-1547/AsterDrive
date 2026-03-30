use chrono::Utc;
use sea_orm::{Set, TransactionTrait};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

use crate::db::repository::{file_repo, folder_repo, user_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::folder_service;

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

#[derive(Serialize, ToSchema)]
pub struct BatchResult {
    pub succeeded: u32,
    pub failed: u32,
    pub errors: Vec<BatchItemError>,
}

#[derive(Serialize, ToSchema)]
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

async fn load_target_folder(
    state: &AppState,
    user_id: i64,
    target_folder_id: Option<i64>,
) -> std::result::Result<Option<folder::Model>, String> {
    let Some(folder_id) = target_folder_id else {
        return Ok(None);
    };

    let folder = folder_repo::find_by_id(&state.db, folder_id)
        .await
        .map_err(|e| e.to_string())?;
    crate::utils::verify_owner(folder.user_id, user_id, "folder").map_err(|e| e.to_string())?;
    if folder.deleted_at.is_some() {
        return Err(
            AsterError::file_not_found(format!("folder #{folder_id} is in trash")).to_string(),
        );
    }
    Ok(Some(folder))
}

async fn load_folder_ancestor_ids(
    state: &AppState,
    target_folder: Option<&folder::Model>,
) -> Result<HashSet<i64>> {
    let mut ancestors = HashSet::new();
    let mut current = target_folder.cloned();

    while let Some(folder) = current {
        ancestors.insert(folder.id);
        current = match folder.parent_id {
            Some(parent_id) => Some(folder_repo::find_by_id(&state.db, parent_id).await?),
            None => None,
        };
    }

    Ok(ancestors)
}

async fn collect_folder_forest(
    state: &AppState,
    user_id: i64,
    root_folder_ids: &[i64],
    include_deleted: bool,
) -> Result<(Vec<file::Model>, Vec<i64>)> {
    if root_folder_ids.is_empty() {
        return Ok((vec![], vec![]));
    }

    let mut files = Vec::new();
    let mut folder_ids = Vec::new();
    let mut seen_folder_ids = HashSet::new();
    let mut frontier: Vec<i64> = root_folder_ids.to_vec();

    while !frontier.is_empty() {
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|id| seen_folder_ids.insert(*id));
        if frontier.is_empty() {
            break;
        }

        folder_ids.extend(frontier.iter().copied());

        if include_deleted {
            files.extend(file_repo::find_all_in_folders(&state.db, &frontier).await?);
            let children = folder_repo::find_all_children_in_parents(&state.db, &frontier).await?;
            frontier = children.into_iter().map(|folder| folder.id).collect();
        } else {
            files.extend(file_repo::find_by_folders(&state.db, user_id, &frontier).await?);
            let children =
                folder_repo::find_children_in_parents(&state.db, user_id, &frontier).await?;
            frontier = children.into_iter().map(|folder| folder.id).collect();
        }
    }

    Ok((files, folder_ids))
}

async fn batch_copy_file_records(
    state: &AppState,
    dest_folder_id: Option<i64>,
    copy_specs: &[(file::Model, String)],
) -> Result<()> {
    if copy_specs.is_empty() {
        return Ok(());
    }

    let user_id = copy_specs[0].0.user_id;
    let now = Utc::now();
    let total_size = copy_specs.iter().try_fold(0i64, |acc, (src, _)| {
        acc.checked_add(src.size).ok_or_else(|| {
            AsterError::internal_error("total copied byte count overflow during batch copy")
        })
    })?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;

    let mut blob_counts: HashMap<i64, i32> = HashMap::new();
    for (src, _) in copy_specs {
        let entry = blob_counts.entry(src.blob_id).or_default();
        *entry = entry.checked_add(1).ok_or_else(|| {
            AsterError::internal_error(format!(
                "blob copy count overflow for blob {} during batch copy",
                src.blob_id
            ))
        })?;
    }

    for (&blob_id, &count) in &blob_counts {
        file_repo::increment_blob_ref_count_by(&txn, blob_id, count).await?;
    }

    let models: Vec<file::ActiveModel> = copy_specs
        .iter()
        .map(|(src, dest_name)| file::ActiveModel {
            name: Set(dest_name.clone()),
            folder_id: Set(dest_folder_id),
            blob_id: Set(src.blob_id),
            size: Set(src.size),
            user_id: Set(src.user_id),
            mime_type: Set(src.mime_type.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .collect();
    file_repo::create_many(&txn, models).await?;
    user_repo::update_storage_used(&txn, user_id, total_size).await?;

    txn.commit().await.map_err(AsterError::from)?;
    Ok(())
}

/// 批量删除（软删除 -> 回收站）
pub async fn batch_delete(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
) -> Result<BatchResult> {
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
        if let Err(err) = crate::utils::verify_owner(file.user_id, user_id, "file") {
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
        if let Err(err) = crate::utils::verify_owner(folder.user_id, user_id, "folder") {
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
        let (tree_files, tree_folder_ids) =
            collect_folder_forest(state, user_id, &root_folder_ids_to_delete, false).await?;
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

/// 批量移动（target_folder_id = None 表示移到根目录）
pub async fn batch_move(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    let mut result = BatchResult::new();
    let file_map = build_file_map(file_repo::find_by_ids(&state.db, file_ids).await?);
    let folder_map = build_folder_map(folder_repo::find_by_ids(&state.db, folder_ids).await?);

    let (target_folder, target_error) =
        match load_target_folder(state, user_id, target_folder_id).await {
            Ok(folder) => (folder, None),
            Err(error) => (None, Some(error)),
        };

    let mut target_file_names = HashMap::new();
    let mut target_folder_names = HashMap::new();
    let mut target_ancestor_ids = HashSet::new();
    if target_error.is_none() {
        target_file_names = file_repo::find_by_folder(&state.db, user_id, target_folder_id)
            .await?
            .into_iter()
            .map(|file| (file.name, file.id))
            .collect();
        target_folder_names = folder_repo::find_children(&state.db, user_id, target_folder_id)
            .await?
            .into_iter()
            .map(|folder| (folder.name, folder.id))
            .collect();
        target_ancestor_ids = load_folder_ancestor_ids(state, target_folder.as_ref()).await?;
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
        if let Err(err) = crate::utils::verify_owner(file.user_id, user_id, "file") {
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
        if file.deleted_at.is_none() {
            target_file_names.insert(file.name.clone(), file.id);
        }
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
        if let Err(err) = crate::utils::verify_owner(folder.user_id, user_id, "folder") {
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
        if folder.deleted_at.is_none() {
            target_folder_names.insert(folder.name.clone(), folder.id);
        }
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

/// 批量复制（target_folder_id = None 表示复制到根目录）
///
/// 使用 `copy_file` / `copy_folder` 高层函数，自动处理：
/// - 权限检查
/// - 副本命名（冲突时递增 "Copy of ..."）
/// - blob ref_count 更新
/// - 配额检查
pub async fn batch_copy(
    state: &AppState,
    user_id: i64,
    file_ids: &[i64],
    folder_ids: &[i64],
    target_folder_id: Option<i64>,
) -> Result<BatchResult> {
    let mut result = BatchResult::new();
    let file_map = build_file_map(file_repo::find_by_ids(&state.db, file_ids).await?);
    let (target_folder, target_error) =
        match load_target_folder(state, user_id, target_folder_id).await {
            Ok(folder) => (folder, None),
            Err(error) => (None, Some(error)),
        };

    let mut reserved_file_names: HashSet<String> = if target_error.is_none() {
        file_repo::find_by_folder(&state.db, user_id, target_folder_id)
            .await?
            .into_iter()
            .map(|file| file.name)
            .collect()
    } else {
        HashSet::new()
    };

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let mut planned_storage_used = user.storage_used;
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
        if let Err(err) = crate::utils::verify_owner(file.user_id, user_id, "file") {
            result.record_failure("file", id, err.to_string());
            continue;
        }
        if let Some(error) = target_error.as_ref() {
            result.record_failure("file", id, error.clone());
            continue;
        }
        if user.storage_quota > 0 && planned_storage_used + file.size > user.storage_quota {
            result.record_failure(
                "file",
                id,
                AsterError::storage_quota_exceeded(format!(
                    "quota {}, used {}, need {}",
                    user.storage_quota, planned_storage_used, file.size
                ))
                .to_string(),
            );
            continue;
        }

        let dest_name = reserve_unique_name(&mut reserved_file_names, &file.name);
        planned_storage_used += file.size;
        result.record_success();
        file_copy_specs.push((file.clone(), dest_name));
    }

    if !file_copy_specs.is_empty() {
        batch_copy_file_records(state, target_folder_id, &file_copy_specs).await?;
    }

    let _ = target_folder;
    for &id in folder_ids {
        match folder_service::copy_folder(state, id, user_id, target_folder_id).await {
            Ok(_) => result.record_success(),
            Err(e) => result.record_failure("folder", id, e.to_string()),
        }
    }

    Ok(result)
}
