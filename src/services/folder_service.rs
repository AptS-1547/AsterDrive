use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set, TransactionTrait};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::db::repository::{file_repo, folder_repo, share_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};
use crate::types::NullablePatch;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderAncestorItem {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FileListItem {
    pub id: i64,
    pub name: String,
    pub folder_id: Option<i64>,
    pub blob_id: i64,
    pub size: i64,
    pub user_id: i64,
    pub mime_type: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderListItem {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub user_id: i64,
    pub policy_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FileCursor {
    /// 排序字段值（序列化为字符串）
    pub value: String,
    pub id: i64,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderContents {
    pub folders: Vec<FolderListItem>,
    pub files: Vec<FileListItem>,
    pub folders_total: u64,
    pub files_total: u64,
    /// 下一页 cursor，None 表示已到最后一页
    pub next_file_cursor: Option<FileCursor>,
}

pub fn build_file_list_items(
    files: Vec<file::Model>,
    shared_file_ids: &HashSet<i64>,
) -> Vec<FileListItem> {
    files
        .into_iter()
        .map(|file| FileListItem {
            id: file.id,
            name: file.name,
            folder_id: file.folder_id,
            blob_id: file.blob_id,
            size: file.size,
            user_id: file.user_id,
            mime_type: file.mime_type,
            created_at: file.created_at,
            updated_at: file.updated_at,
            is_locked: file.is_locked,
            is_shared: shared_file_ids.contains(&file.id),
        })
        .collect()
}

pub fn build_folder_list_items(
    folders: Vec<folder::Model>,
    shared_folder_ids: &HashSet<i64>,
) -> Vec<FolderListItem> {
    folders
        .into_iter()
        .map(|folder| FolderListItem {
            id: folder.id,
            name: folder.name,
            parent_id: folder.parent_id,
            user_id: folder.user_id,
            policy_id: folder.policy_id,
            created_at: folder.created_at,
            updated_at: folder.updated_at,
            is_locked: folder.is_locked,
            is_shared: shared_folder_ids.contains(&folder.id),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn build_folder_contents(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folders: Vec<folder::Model>,
    folders_total: u64,
    files: Vec<file::Model>,
    files_total: u64,
    sort_by: crate::api::pagination::SortBy,
    file_limit: u64,
) -> Result<FolderContents> {
    let next_file_cursor = if files.len() as u64 == file_limit && file_limit > 0 {
        files.last().map(|f| FileCursor {
            value: crate::api::pagination::SortBy::cursor_value(f, sort_by),
            id: f.id,
        })
    } else {
        None
    };

    let file_ids: Vec<i64> = files.iter().map(|file| file.id).collect();
    let folder_ids: Vec<i64> = folders.iter().map(|folder| folder.id).collect();
    let (shared_file_ids, shared_folder_ids) = match scope {
        WorkspaceStorageScope::Personal { user_id } => (
            share_repo::find_active_file_ids(&state.db, user_id, &file_ids).await?,
            share_repo::find_active_folder_ids(&state.db, user_id, &folder_ids).await?,
        ),
        WorkspaceStorageScope::Team { team_id, .. } => (
            share_repo::find_active_team_file_ids(&state.db, team_id, &file_ids).await?,
            share_repo::find_active_team_folder_ids(&state.db, team_id, &folder_ids).await?,
        ),
    };

    Ok(FolderContents {
        folders: build_folder_list_items(folders, &shared_folder_ids),
        files: build_file_list_items(files, &shared_file_ids),
        folders_total,
        files_total,
        next_file_cursor,
    })
}

fn ensure_folder_model_in_scope(
    folder: &folder::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    workspace_storage_service::ensure_active_folder_scope(folder, scope)
}

pub(crate) async fn create_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    name: &str,
    parent_id: Option<i64>,
) -> Result<folder::Model> {
    if let WorkspaceStorageScope::Team {
        team_id,
        actor_user_id,
    } = scope
    {
        workspace_storage_service::require_team_access(state, team_id, actor_user_id).await?;
    }

    crate::utils::validate_name(name)?;

    if let Some(pid) = parent_id {
        workspace_storage_service::verify_folder_access(state, scope, pid).await?;
    }

    let exists = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_by_name_in_parent(&state.db, user_id, parent_id, name)
                .await?
                .is_some()
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_by_name_in_team_parent(&state.db, team_id, parent_id, name)
                .await?
                .is_some()
        }
    };

    if exists {
        return Err(AsterError::validation_error(format!(
            "folder '{}' already exists in this location",
            name
        )));
    }

    let now = Utc::now();
    folder_repo::create(
        &state.db,
        folder::ActiveModel {
            name: Set(name.to_string()),
            parent_id: Set(parent_id),
            team_id: Set(scope.team_id()),
            user_id: Set(scope.actor_user_id()),
            policy_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub async fn create(
    state: &AppState,
    user_id: i64,
    name: &str,
    parent_id: Option<i64>,
) -> Result<folder::Model> {
    create_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        name,
        parent_id,
    )
    .await
}

async fn ensure_folder_in_parent<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<folder::Model> {
    crate::utils::validate_name(name)?;

    if let Some(existing) =
        folder_repo::find_by_name_in_parent(db, user_id, parent_id, name).await?
    {
        return Ok(existing);
    }

    let now = Utc::now();
    let model = folder::ActiveModel {
        name: Set(name.to_string()),
        parent_id: Set(parent_id),
        user_id: Set(user_id),
        policy_id: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    match folder_repo::create(db, model).await {
        Ok(created) => Ok(created),
        Err(err) => {
            if let Some(existing) =
                folder_repo::find_by_name_in_parent(db, user_id, parent_id, name).await?
            {
                Ok(existing)
            } else {
                Err(err)
            }
        }
    }
}

async fn load_folder_chain_map(
    db: &sea_orm::DatabaseConnection,
    folder_ids: &[i64],
) -> Result<HashMap<i64, folder::Model>> {
    let mut loaded = HashMap::new();
    let mut frontier: Vec<i64> = folder_ids.to_vec();

    while !frontier.is_empty() {
        frontier.retain(|id| !loaded.contains_key(id));
        frontier.sort_unstable();
        frontier.dedup();
        if frontier.is_empty() {
            break;
        }

        let rows = folder_repo::find_by_ids(db, &frontier).await?;
        let mut found = HashSet::with_capacity(rows.len());
        let mut next = Vec::new();

        for row in rows {
            found.insert(row.id);
            if let Some(pid) = row.parent_id
                && !loaded.contains_key(&pid)
            {
                next.push(pid);
            }
            loaded.insert(row.id, row);
        }

        if let Some(missing) = frontier.iter().find(|id| !found.contains(id)) {
            return Err(AsterError::record_not_found(format!("folder #{missing}")));
        }

        frontier = next;
    }

    Ok(loaded)
}

pub async fn build_folder_paths(
    db: &sea_orm::DatabaseConnection,
    folder_ids: &[i64],
) -> Result<HashMap<i64, String>> {
    let chain_map = load_folder_chain_map(db, folder_ids).await?;
    let mut paths = HashMap::with_capacity(folder_ids.len());

    for &folder_id in folder_ids {
        let mut parts = Vec::new();
        let mut current_id = Some(folder_id);
        while let Some(id) = current_id {
            let folder = chain_map
                .get(&id)
                .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))?;
            parts.push(folder.name.clone());
            current_id = folder.parent_id;
        }
        parts.reverse();
        paths.insert(folder_id, format!("/{}", parts.join("/")));
    }

    Ok(paths)
}

pub async fn verify_folder_in_scope(
    db: &sea_orm::DatabaseConnection,
    folder_id: i64,
    root_folder_id: i64,
) -> Result<()> {
    if folder_id == root_folder_id {
        return Ok(());
    }

    let chain_map = load_folder_chain_map(db, &[folder_id]).await?;
    let mut current_id = Some(folder_id);
    while let Some(id) = current_id {
        let folder = chain_map
            .get(&id)
            .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))?;
        if folder.parent_id == Some(root_folder_id) {
            return Ok(());
        }
        current_id = folder.parent_id;
    }

    Err(AsterError::auth_forbidden(
        "folder is outside shared folder scope",
    ))
}

pub(crate) fn ensure_personal_folder_scope(folder: &folder::Model) -> Result<()> {
    if folder.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "folder belongs to a team workspace",
        ));
    }
    Ok(())
}

/// 校验目标文件夹存在、归属当前用户且未被删除
pub async fn verify_folder_access(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_personal_folder_scope(&folder)?;
    crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
    if folder.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    Ok(())
}

fn file_matches_scope(file: &file::Model, scope: WorkspaceStorageScope) -> bool {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file.team_id.is_none() && file.user_id == user_id
        }
        WorkspaceStorageScope::Team { team_id, .. } => file.team_id == Some(team_id),
    }
}

fn folder_matches_scope(folder: &folder::Model, scope: WorkspaceStorageScope) -> bool {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder.team_id.is_none() && folder.user_id == user_id
        }
        WorkspaceStorageScope::Team { team_id, .. } => folder.team_id == Some(team_id),
    }
}

pub(crate) async fn collect_folder_forest_in_scope(
    db: &sea_orm::DatabaseConnection,
    scope: WorkspaceStorageScope,
    root_folder_ids: &[i64],
    include_deleted: bool,
) -> Result<(Vec<file::Model>, Vec<i64>)> {
    if root_folder_ids.is_empty() {
        return Ok((vec![], vec![]));
    }

    let mut files = Vec::new();
    let mut folder_ids = Vec::new();
    let mut seen_folder_ids = HashSet::new();
    let mut frontier = root_folder_ids.to_vec();

    while !frontier.is_empty() {
        frontier.sort_unstable();
        frontier.dedup();
        frontier.retain(|id| seen_folder_ids.insert(*id));
        if frontier.is_empty() {
            break;
        }

        folder_ids.extend(frontier.iter().copied());

        if include_deleted {
            files.extend(
                file_repo::find_all_in_folders(db, &frontier)
                    .await?
                    .into_iter()
                    .filter(|file| file_matches_scope(file, scope)),
            );
            frontier = folder_repo::find_all_children_in_parents(db, &frontier)
                .await?
                .into_iter()
                .filter(|folder| folder_matches_scope(folder, scope))
                .map(|folder| folder.id)
                .collect();
            continue;
        }

        frontier = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                files.extend(file_repo::find_by_folders(db, user_id, &frontier).await?);
                folder_repo::find_children_in_parents(db, user_id, &frontier)
                    .await?
                    .into_iter()
                    .map(|folder| folder.id)
                    .collect()
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                files.extend(file_repo::find_by_team_folders(db, team_id, &frontier).await?);
                folder_repo::find_team_children_in_parents(db, team_id, &frontier)
                    .await?
                    .into_iter()
                    .map(|folder| folder.id)
                    .collect()
            }
        };
    }

    Ok((files, folder_ids))
}

pub(crate) async fn collect_folder_tree_in_scope(
    db: &sea_orm::DatabaseConnection,
    scope: WorkspaceStorageScope,
    folder_id: i64,
    include_deleted: bool,
) -> Result<(Vec<file::Model>, Vec<i64>)> {
    collect_folder_forest_in_scope(db, scope, &[folder_id], include_deleted).await
}

pub(crate) async fn delete_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<()> {
    let folder = workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;
    if folder.is_locked {
        return Err(AsterError::resource_locked("folder is locked"));
    }

    let (files, folder_ids) =
        collect_folder_tree_in_scope(&state.db, scope, folder_id, false).await?;
    let file_ids: Vec<i64> = files.into_iter().map(|f| f.id).collect();
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    file_repo::soft_delete_many(&txn, &file_ids, now).await?;
    folder_repo::soft_delete_many(&txn, &folder_ids, now).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(())
}

pub(crate) async fn update_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
    name: Option<String>,
    parent_id: NullablePatch<i64>,
    policy_id: NullablePatch<i64>,
) -> Result<folder::Model> {
    let db = &state.db;
    let f = workspace_storage_service::verify_folder_access(state, scope, id).await?;
    if f.is_locked {
        return Err(AsterError::resource_locked("folder is locked"));
    }

    if let NullablePatch::Value(pid) = parent_id {
        if pid == id {
            return Err(AsterError::validation_error(
                "cannot move folder into itself",
            ));
        }
        workspace_storage_service::verify_folder_access(state, scope, pid).await?;
        let mut cursor = Some(pid);
        while let Some(cur_id) = cursor {
            if cur_id == id {
                return Err(AsterError::validation_error(
                    "cannot move folder into its own subfolder",
                ));
            }
            let cur = folder_repo::find_by_id(db, cur_id).await?;
            ensure_folder_model_in_scope(&cur, scope)?;
            cursor = cur.parent_id;
        }
    }

    if let Some(ref n) = name {
        crate::utils::validate_name(n)?;
    }

    let target_parent = match parent_id {
        NullablePatch::Absent => f.parent_id,
        NullablePatch::Null => None,
        NullablePatch::Value(pid) => Some(pid),
    };
    let final_name = name.as_deref().unwrap_or(&f.name);
    let existing = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_by_name_in_parent(db, user_id, target_parent, final_name).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_by_name_in_team_parent(db, team_id, target_parent, final_name).await?
        }
    };
    if let Some(existing) = existing
        && existing.id != id
    {
        return Err(AsterError::validation_error(format!(
            "folder '{}' already exists in this location",
            final_name
        )));
    }

    let mut active: folder::ActiveModel = f.into();
    if let Some(n) = name {
        active.name = Set(n);
    }
    match parent_id {
        NullablePatch::Absent => {}
        NullablePatch::Null => active.parent_id = Set(None),
        NullablePatch::Value(pid) => active.parent_id = Set(Some(pid)),
    }
    match policy_id {
        NullablePatch::Absent => {}
        NullablePatch::Null => active.policy_id = Set(None),
        NullablePatch::Value(pid) => active.policy_id = Set(Some(pid)),
    }
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

pub(crate) async fn get_ancestors_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<Vec<FolderAncestorItem>> {
    workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;

    let mut path = Vec::new();
    let mut current_id = folder_id;

    loop {
        let folder = folder_repo::find_by_id(&state.db, current_id).await?;
        ensure_folder_model_in_scope(&folder, scope)?;
        path.push(FolderAncestorItem {
            id: folder.id,
            name: folder.name,
        });
        match folder.parent_id {
            Some(pid) => current_id = pid,
            None => break,
        }
    }

    path.reverse();
    Ok(path)
}

pub(crate) async fn set_lock_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
    locked: bool,
) -> Result<folder::Model> {
    use crate::services::lock_service;
    use crate::types::EntityType;

    workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;

    if locked {
        lock_service::lock(
            state,
            EntityType::Folder,
            folder_id,
            Some(scope.actor_user_id()),
            None,
            None,
        )
        .await?;
    } else {
        lock_service::unlock(state, EntityType::Folder, folder_id, scope.actor_user_id()).await?;
    }

    workspace_storage_service::verify_folder_access(state, scope, folder_id).await
}

pub async fn resolve_upload_path(
    state: &AppState,
    user_id: i64,
    base_folder_id: Option<i64>,
    relative_path: &str,
) -> Result<(Option<i64>, String)> {
    // 校验 base_folder_id 归属
    if let Some(fid) = base_folder_id {
        verify_folder_access(state, user_id, fid).await?;
    }

    // Reject paths with empty segments (e.g. "docs//bad.txt")
    if relative_path.split('/').any(|s| s.is_empty()) {
        return Err(AsterError::validation_error(
            "relative_path contains empty path segments",
        ));
    }
    let segments: Vec<&str> = relative_path.split('/').collect();
    let filename = segments
        .last()
        .ok_or_else(|| AsterError::validation_error("relative_path cannot be empty"))?;
    crate::utils::validate_name(filename)?;

    if segments.len() == 1 {
        return Ok((base_folder_id, (*filename).to_string()));
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let mut current_parent = base_folder_id;

    for segment in &segments[..segments.len() - 1] {
        let folder = ensure_folder_in_parent(&txn, user_id, current_parent, segment).await?;
        current_parent = Some(folder.id);
    }

    txn.commit().await.map_err(AsterError::from)?;
    Ok((current_parent, (*filename).to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn list_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<FolderContents> {
    if let WorkspaceStorageScope::Team {
        team_id,
        actor_user_id,
    } = scope
    {
        workspace_storage_service::require_team_access(state, team_id, actor_user_id).await?;
    }

    if let Some(parent_id) = parent_id {
        workspace_storage_service::verify_folder_access(state, scope, parent_id).await?;
    }

    let (folders, folders_total, files, files_total) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            let (folders, folders_total) = if folder_limit == 0 {
                (
                    vec![],
                    folder_repo::find_children_paginated(
                        &state.db, user_id, parent_id, 0, 0, sort_by, sort_order,
                    )
                    .await?
                    .1,
                )
            } else {
                folder_repo::find_children_paginated(
                    &state.db,
                    user_id,
                    parent_id,
                    folder_limit,
                    folder_offset,
                    sort_by,
                    sort_order,
                )
                .await?
            };

            let (files, files_total) = if file_limit == 0 {
                (
                    vec![],
                    file_repo::find_by_folder_cursor(
                        &state.db, user_id, parent_id, 0, None, sort_by, sort_order,
                    )
                    .await?
                    .1,
                )
            } else {
                file_repo::find_by_folder_cursor(
                    &state.db,
                    user_id,
                    parent_id,
                    file_limit,
                    file_cursor,
                    sort_by,
                    sort_order,
                )
                .await?
            };

            (folders, folders_total, files, files_total)
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            let (folders, folders_total) = if folder_limit == 0 {
                (
                    vec![],
                    folder_repo::find_team_children_paginated(
                        &state.db, team_id, parent_id, 0, 0, sort_by, sort_order,
                    )
                    .await?
                    .1,
                )
            } else {
                folder_repo::find_team_children_paginated(
                    &state.db,
                    team_id,
                    parent_id,
                    folder_limit,
                    folder_offset,
                    sort_by,
                    sort_order,
                )
                .await?
            };

            let (files, files_total) = if file_limit == 0 {
                (
                    vec![],
                    file_repo::find_by_team_folder_cursor(
                        &state.db, team_id, parent_id, 0, None, sort_by, sort_order,
                    )
                    .await?
                    .1,
                )
            } else {
                file_repo::find_by_team_folder_cursor(
                    &state.db,
                    team_id,
                    parent_id,
                    file_limit,
                    file_cursor,
                    sort_by,
                    sort_order,
                )
                .await?
            };

            (folders, folders_total, files, files_total)
        }
    };

    build_folder_contents(
        state,
        scope,
        folders,
        folders_total,
        files,
        files_total,
        sort_by,
        file_limit,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn list(
    state: &AppState,
    user_id: i64,
    parent_id: Option<i64>,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<FolderContents> {
    list_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        parent_id,
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
        sort_by,
        sort_order,
    )
    .await
}

/// 删除文件夹（软删除 → 回收站，递归标记子项）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    delete_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub async fn update(
    state: &AppState,
    id: i64,
    user_id: i64,
    name: Option<String>,
    parent_id: NullablePatch<i64>,
    policy_id: NullablePatch<i64>,
) -> Result<folder::Model> {
    update_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        name,
        parent_id,
        policy_id,
    )
    .await
}

/// 移动文件夹到指定父文件夹（None = 根目录）
///
/// 与 `update()` 的区别：`update()` 用 `NullablePatch<i64>` 区分
/// “未传字段”和“显式传 null”，而本函数的 `target_parent_id: None`
/// 明确表示“移到根目录”。
pub async fn move_folder(
    state: &AppState,
    id: i64,
    user_id: i64,
    target_parent_id: Option<i64>,
) -> Result<folder::Model> {
    update_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        None,
        match target_parent_id {
            Some(parent_id) => NullablePatch::Value(parent_id),
            None => NullablePatch::Null,
        },
        NullablePatch::Absent,
    )
    .await
}

pub(crate) fn recursive_copy_folder_in_scope<'a>(
    state: &'a AppState,
    scope: WorkspaceStorageScope,
    src_folder_id: i64,
    dest_parent_id: Option<i64>,
    dest_name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<folder::Model>> + Send + 'a>> {
    Box::pin(async move {
        let db = &state.db;
        let now = Utc::now();
        let src_folder = folder_repo::find_by_id(db, src_folder_id).await?;
        ensure_folder_model_in_scope(&src_folder, scope)?;

        let new_folder = folder_repo::create(
            db,
            folder::ActiveModel {
                name: Set(dest_name.to_string()),
                parent_id: Set(dest_parent_id),
                team_id: Set(scope.team_id()),
                user_id: Set(scope.actor_user_id()),
                policy_id: Set(src_folder.policy_id),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        let files = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                file_repo::find_by_folder(db, user_id, Some(src_folder_id)).await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                file_repo::find_by_team_folder(db, team_id, Some(src_folder_id)).await?
            }
        };
        crate::services::file_service::batch_duplicate_file_records_in_scope(
            state,
            scope,
            &files,
            Some(new_folder.id),
        )
        .await?;

        let children = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                folder_repo::find_children(db, user_id, Some(src_folder_id)).await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                folder_repo::find_team_children(db, team_id, Some(src_folder_id)).await?
            }
        };
        for child in children {
            recursive_copy_folder_in_scope(
                state,
                scope,
                child.id,
                Some(new_folder.id),
                &child.name,
            )
            .await?;
        }

        Ok(new_folder)
    })
}

pub(crate) async fn copy_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    src_id: i64,
    dest_parent_id: Option<i64>,
) -> Result<folder::Model> {
    let db = &state.db;
    let src = workspace_storage_service::verify_folder_access(state, scope, src_id).await?;

    if let Some(parent_id) = dest_parent_id {
        workspace_storage_service::verify_folder_access(state, scope, parent_id).await?;

        let mut cursor = Some(parent_id);
        while let Some(cur_id) = cursor {
            if cur_id == src_id {
                return Err(AsterError::validation_error(
                    "cannot copy folder into its own subfolder",
                ));
            }
            let current = folder_repo::find_by_id(db, cur_id).await?;
            ensure_folder_model_in_scope(&current, scope)?;
            cursor = current.parent_id;
        }
    }

    let mut dest_name = src.name.clone();
    loop {
        let exists = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                folder_repo::find_by_name_in_parent(db, user_id, dest_parent_id, &dest_name)
                    .await?
                    .is_some()
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                folder_repo::find_by_name_in_team_parent(db, team_id, dest_parent_id, &dest_name)
                    .await?
                    .is_some()
            }
        };

        if !exists {
            break;
        }
        dest_name = crate::utils::next_copy_name(&dest_name);
    }

    recursive_copy_folder_in_scope(state, scope, src_id, dest_parent_id, &dest_name).await
}

/// 复制文件夹（递归复制所有文件和子文件夹）
///
/// `dest_parent_id = None` 表示复制到根目录。
pub async fn copy_folder(
    state: &AppState,
    src_id: i64,
    user_id: i64,
    dest_parent_id: Option<i64>,
) -> Result<folder::Model> {
    copy_folder_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        src_id,
        dest_parent_id,
    )
    .await
}

/// 列出文件夹内容（无用户校验，用于分享链接）
#[allow(clippy::too_many_arguments)]
pub async fn list_shared(
    state: &AppState,
    folder_id: i64,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<FolderContents> {
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    if let Some(team_id) = folder.team_id {
        let (folders, folders_total) = folder_repo::find_team_children_paginated(
            &state.db,
            team_id,
            Some(folder_id),
            folder_limit,
            folder_offset,
            sort_by,
            sort_order,
        )
        .await?;
        let (files, files_total) = file_repo::find_by_team_folder_cursor(
            &state.db,
            team_id,
            Some(folder_id),
            file_limit,
            file_cursor,
            sort_by,
            sort_order,
        )
        .await?;

        let next_file_cursor = if files.len() as u64 == file_limit && file_limit > 0 {
            files.last().map(|f| FileCursor {
                value: crate::api::pagination::SortBy::cursor_value(f, sort_by),
                id: f.id,
            })
        } else {
            None
        };

        let file_ids: Vec<i64> = files.iter().map(|file| file.id).collect();
        let folder_ids: Vec<i64> = folders.iter().map(|folder| folder.id).collect();
        let shared_file_ids =
            share_repo::find_active_team_file_ids(&state.db, team_id, &file_ids).await?;
        let shared_folder_ids =
            share_repo::find_active_team_folder_ids(&state.db, team_id, &folder_ids).await?;

        Ok(FolderContents {
            folders: build_folder_list_items(folders, &shared_folder_ids),
            files: build_file_list_items(files, &shared_file_ids),
            folders_total,
            files_total,
            next_file_cursor,
        })
    } else {
        ensure_personal_folder_scope(&folder)?;
        let (folders, folders_total) = folder_repo::find_children_paginated(
            &state.db,
            folder.user_id,
            Some(folder_id),
            folder_limit,
            folder_offset,
            sort_by,
            sort_order,
        )
        .await?;
        let (files, files_total) = file_repo::find_by_folder_cursor(
            &state.db,
            folder.user_id,
            Some(folder_id),
            file_limit,
            file_cursor,
            sort_by,
            sort_order,
        )
        .await?;

        build_folder_contents(
            state,
            WorkspaceStorageScope::Personal {
                user_id: folder.user_id,
            },
            folders,
            folders_total,
            files,
            files_total,
            sort_by,
            file_limit,
        )
        .await
    }
}

/// 获取文件夹的祖先链（从根下第一层到当前文件夹）
pub async fn get_ancestors(
    state: &AppState,
    user_id: i64,
    folder_id: i64,
) -> Result<Vec<FolderAncestorItem>> {
    get_ancestors_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
    )
    .await
}

// ── Lock ─────────────────────────────────────────────────────────────

/// 设置/解除文件夹锁，返回更新后的文件夹信息
pub async fn set_lock(
    state: &AppState,
    folder_id: i64,
    user_id: i64,
    locked: bool,
) -> Result<folder::Model> {
    set_lock_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        folder_id,
        locked,
    )
    .await
}
