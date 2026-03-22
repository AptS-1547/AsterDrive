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
    crate::utils::validate_name(name)?;

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
pub async fn copy_folder(
    state: &AppState,
    src_id: i64,
    user_id: i64,
    dest_parent_id: Option<i64>,
) -> Result<folder::Model> {
    let db = &state.db;
    let f = folder_repo::find_by_id(db, src_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "folder")?;

    // 副本命名：目标无冲突保留原名，有冲突则递增
    let dest = dest_parent_id.or(f.parent_id);
    let mut dest_name = f.name.clone();
    while folder_repo::find_by_name_in_parent(db, user_id, dest, &dest_name)
        .await?
        .is_some()
    {
        dest_name = crate::utils::next_copy_name(&dest_name);
    }

    crate::services::webdav_service::recursive_copy_folder(state, user_id, src_id, dest, &dest_name)
        .await
}

/// 列出文件夹内容（无用户校验，用于分享链接）
pub async fn list_shared(state: &AppState, folder_id: i64) -> Result<FolderContents> {
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    let folders = folder_repo::find_children(&state.db, folder.user_id, Some(folder_id)).await?;
    let files = file_repo::find_by_folder(&state.db, folder.user_id, Some(folder_id)).await?;
    Ok(FolderContents { folders, files })
}
