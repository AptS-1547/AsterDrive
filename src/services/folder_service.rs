use chrono::Utc;
use sea_orm::Set;
use serde::Serialize;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

#[derive(Serialize)]
pub struct FolderContents {
    pub folders: Vec<folder::Model>,
    pub files: Vec<file::Model>,
}

pub async fn create(
    state: &AppState,
    user_id: i64,
    name: &str,
    parent_id: Option<i64>,
) -> Result<folder::Model> {
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

pub async fn list(
    state: &AppState,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<FolderContents> {
    let folders = folder_repo::find_children(&state.db, user_id, parent_id)
        .await?
        .into_iter()
        .filter(|f| !crate::utils::is_hidden_name(&f.name))
        .collect();
    let files = file_repo::find_by_folder(&state.db, user_id, parent_id)
        .await?
        .into_iter()
        .filter(|f| !crate::utils::is_hidden_name(&f.name))
        .collect();
    Ok(FolderContents { folders, files })
}

/// 删除文件夹（软删除 → 回收站，递归标记子项）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let folder = folder_repo::find_by_id(&state.db, id).await?;
    if folder.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your folder"));
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
    let f = folder_repo::find_by_id(&state.db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your folder"));
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
    active.update(&state.db).await.map_err(AsterError::from)
}

/// 列出文件夹内容（无用户校验，用于分享链接）
pub async fn list_shared(state: &AppState, folder_id: i64) -> Result<FolderContents> {
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    let folders = folder_repo::find_children(&state.db, folder.user_id, Some(folder_id)).await?;
    let files = file_repo::find_by_folder(&state.db, folder.user_id, Some(folder_id)).await?;
    Ok(FolderContents { folders, files })
}
