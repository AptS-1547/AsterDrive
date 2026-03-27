use chrono::Utc;
use sea_orm::{ConnectionTrait, Set, TransactionTrait};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

use crate::db::repository::{file_repo, folder_repo, share_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

#[derive(Serialize, ToSchema)]
pub struct FolderAncestorItem {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct FileListItem {
    pub id: i64,
    pub name: String,
    pub folder_id: Option<i64>,
    pub blob_id: i64,
    pub size: i64,
    pub user_id: i64,
    pub mime_type: String,
    #[schema(value_type = String)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct FolderListItem {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub user_id: i64,
    pub policy_id: Option<i64>,
    #[schema(value_type = String)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Serialize, ToSchema)]
pub struct FileCursor {
    /// 排序字段值（序列化为字符串）
    pub value: String,
    pub id: i64,
}

#[derive(Serialize, ToSchema)]
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

async fn build_folder_contents(
    state: &AppState,
    user_id: i64,
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
    let shared_file_ids = share_repo::find_active_file_ids(&state.db, user_id, &file_ids).await?;
    let shared_folder_ids =
        share_repo::find_active_folder_ids(&state.db, user_id, &folder_ids).await?;

    Ok(FolderContents {
        folders: build_folder_list_items(folders, &shared_folder_ids),
        files: build_file_list_items(files, &shared_file_ids),
        folders_total,
        files_total,
        next_file_cursor,
    })
}

pub async fn create(
    state: &AppState,
    user_id: i64,
    name: &str,
    parent_id: Option<i64>,
) -> Result<folder::Model> {
    crate::utils::validate_name(name)?;

    // 校验 parent_id 归属
    if let Some(pid) = parent_id {
        verify_folder_access(state, user_id, pid).await?;
    }

    // 检查同名文件夹
    if folder_repo::find_by_name_in_parent(&state.db, user_id, parent_id, name)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(format!(
            "folder '{}' already exists in this location",
            name
        )));
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
    folder_repo::create(&state.db, model).await
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

/// 校验目标文件夹存在、归属当前用户且未被删除
pub async fn verify_folder_access(state: &AppState, user_id: i64, folder_id: i64) -> Result<()> {
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
    if folder.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    Ok(())
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

    let segments: Vec<&str> = relative_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
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
        let (folders, total) = folder_repo::find_children_paginated(
            &state.db,
            user_id,
            parent_id,
            folder_limit,
            folder_offset,
            sort_by,
            sort_order,
        )
        .await?;
        (folders, total)
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

    build_folder_contents(
        state,
        user_id,
        folders,
        folders_total,
        files,
        files_total,
        sort_by,
        file_limit,
    )
    .await
}

/// 删除文件夹（软删除 → 回收站，递归标记子项）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let folder = folder_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
    if folder.is_locked {
        return Err(AsterError::resource_locked("folder is locked"));
    }
    crate::services::webdav_service::recursive_soft_delete(state, user_id, id).await
}

pub async fn update(
    state: &AppState,
    id: i64,
    user_id: i64,
    name: Option<String>,
    parent_id: Option<i64>,
    policy_id: Option<i64>,
) -> Result<folder::Model> {
    let db = &state.db;
    let f = folder_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("folder is locked"));
    }

    // 目标父文件夹校验
    if let Some(pid) = parent_id {
        // 不能移到自己
        if pid == id {
            return Err(AsterError::validation_error(
                "cannot move folder into itself",
            ));
        }
        let target = folder_repo::find_by_id(db, pid).await?;
        crate::utils::verify_owner(target.user_id, user_id, "folder")?;
        // 循环检测：从目标往上遍历，如果遇到 id 说明是子文件夹
        let mut cursor = Some(pid);
        while let Some(cur_id) = cursor {
            if cur_id == id {
                return Err(AsterError::validation_error(
                    "cannot move folder into its own subfolder",
                ));
            }
            let cur = folder_repo::find_by_id(db, cur_id).await?;
            cursor = cur.parent_id;
        }
    }

    // 文件名验证
    if let Some(ref n) = name {
        crate::utils::validate_name(n)?;
    }

    // 同名冲突检查
    let target_parent = parent_id.or(f.parent_id);
    let final_name = name.as_deref().unwrap_or(&f.name);
    if let Some(existing) =
        folder_repo::find_by_name_in_parent(db, user_id, target_parent, final_name).await?
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
    if let Some(pid) = parent_id {
        active.parent_id = Set(Some(pid));
    }
    if let Some(pid) = policy_id {
        active.policy_id = Set(Some(pid));
    }
    active.updated_at = Set(Utc::now());
    use sea_orm::ActiveModelTrait;
    active.update(db).await.map_err(AsterError::from)
}

/// 移动文件夹到指定父文件夹（None = 根目录）
///
/// 与 `update()` 的区别：`update()` 的 `parent_id: Option<i64>` 中 `None` 表示"不变"，
/// 而本函数的 `target_parent_id: None` 明确表示"移到根目录"。
pub async fn move_folder(
    state: &AppState,
    id: i64,
    user_id: i64,
    target_parent_id: Option<i64>,
) -> Result<folder::Model> {
    let db = &state.db;
    let f = folder_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("folder is locked"));
    }

    // 验证目标父文件夹 + 循环检测
    if let Some(pid) = target_parent_id {
        if pid == id {
            return Err(AsterError::validation_error(
                "cannot move folder into itself",
            ));
        }
        let target = folder_repo::find_by_id(db, pid).await?;
        crate::utils::verify_owner(target.user_id, user_id, "folder")?;
        // 循环检测：从目标往上遍历，如果遇到 id 说明是子文件夹
        let mut cursor = Some(pid);
        while let Some(cur_id) = cursor {
            if cur_id == id {
                return Err(AsterError::validation_error(
                    "cannot move folder into its own subfolder",
                ));
            }
            let cur = folder_repo::find_by_id(db, cur_id).await?;
            cursor = cur.parent_id;
        }
    }

    // 检查同名冲突
    if let Some(existing) =
        folder_repo::find_by_name_in_parent(db, user_id, target_parent_id, &f.name).await?
        && existing.id != id
    {
        return Err(AsterError::validation_error(format!(
            "folder '{}' already exists in target folder",
            f.name
        )));
    }

    let mut active: folder::ActiveModel = f.into();
    active.parent_id = Set(target_parent_id);
    active.updated_at = Set(Utc::now());
    use sea_orm::ActiveModelTrait;
    active.update(db).await.map_err(AsterError::from)
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
    let db = &state.db;
    let f = folder_repo::find_by_id(db, src_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;

    if let Some(parent_id) = dest_parent_id {
        verify_folder_access(state, user_id, parent_id).await?;
    }

    // 副本命名：目标无冲突保留原名，有冲突则递增
    let mut dest_name = f.name.clone();
    while folder_repo::find_by_name_in_parent(db, user_id, dest_parent_id, &dest_name)
        .await?
        .is_some()
    {
        dest_name = crate::utils::next_copy_name(&dest_name);
    }

    crate::services::webdav_service::recursive_copy_folder(
        state,
        user_id,
        src_id,
        dest_parent_id,
        &dest_name,
    )
    .await
}

/// 列出文件夹内容（无用户校验，用于分享链接）
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
        folder.user_id,
        folders,
        folders_total,
        files,
        files_total,
        sort_by,
        file_limit,
    )
    .await
}

/// 获取文件夹的祖先链（从根下第一层到当前文件夹）
pub async fn get_ancestors(
    state: &AppState,
    user_id: i64,
    folder_id: i64,
) -> Result<Vec<FolderAncestorItem>> {
    let ancestors = folder_repo::find_ancestors(&state.db, user_id, folder_id).await?;
    Ok(ancestors
        .into_iter()
        .map(|(id, name)| FolderAncestorItem { id, name })
        .collect())
}
