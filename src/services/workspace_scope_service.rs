//! 统一描述“当前操作落在哪个工作空间里”。
//!
//! 个人空间和团队空间共用大部分文件主链路，但权限和资源归属并不完全相同。
//! 这里负责把 scope 相关规则收口，避免每个上层 service 都自己拼一套
//! `user_id` / `team_id` / `actor_user_id` 判断。

use crate::db::repository::{file_repo, folder_repo, team_member_repo, team_repo};
use crate::entities::{file, folder, team};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;

/// scope 同时表达“资源属于哪个空间”和“是谁在操作”。
///
/// 个人空间里两者通常是同一个人；团队空间里则必须同时保留 `team_id`
/// 和 `actor_user_id`，否则后续无法同时做成员校验和归属校验。
#[derive(Clone, Copy, Debug)]
pub(crate) enum WorkspaceStorageScope {
    Personal { user_id: i64 },
    Team { team_id: i64, actor_user_id: i64 },
}

impl WorkspaceStorageScope {
    pub(crate) fn actor_user_id(self) -> i64 {
        match self {
            Self::Personal { user_id } => user_id,
            Self::Team { actor_user_id, .. } => actor_user_id,
        }
    }

    pub(crate) fn team_id(self) -> Option<i64> {
        match self {
            Self::Personal { .. } => None,
            Self::Team { team_id, .. } => Some(team_id),
        }
    }
}

pub(crate) async fn require_scope_access(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    // 个人空间天然只需要“用户正在操作自己的空间”这个前提；
    // 团队空间则必须先确认 actor 当前仍然是团队成员。
    if let WorkspaceStorageScope::Team {
        team_id,
        actor_user_id,
    } = scope
    {
        require_team_access(state, team_id, actor_user_id).await?;
    }

    Ok(())
}

pub(crate) fn ensure_personal_file_scope(file: &file::Model) -> Result<()> {
    if file.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "file belongs to a team workspace",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_personal_folder_scope(folder: &folder::Model) -> Result<()> {
    if folder.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "folder belongs to a team workspace",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_file_scope(file: &file::Model, scope: WorkspaceStorageScope) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            ensure_personal_file_scope(file)?;
            crate::utils::verify_owner(file.user_id, user_id, "file")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if file.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden("file is outside team workspace"));
            }
        }
    }

    Ok(())
}

pub(crate) fn ensure_active_file_scope(
    file: &file::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    ensure_file_scope(file, scope)?;

    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{} is in trash",
            file.id
        )));
    }

    Ok(())
}

pub(crate) fn ensure_folder_scope(
    folder: &folder::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            ensure_personal_folder_scope(folder)?;
            crate::utils::verify_owner(folder.user_id, user_id, "folder")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if folder.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden(
                    "folder is outside team workspace",
                ));
            }
        }
    }

    Ok(())
}

pub(crate) fn ensure_active_folder_scope(
    folder: &folder::Model,
    scope: WorkspaceStorageScope,
) -> Result<()> {
    ensure_folder_scope(folder, scope)?;

    if folder.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "folder #{} is in trash",
            folder.id
        )));
    }

    Ok(())
}

pub(crate) async fn require_team_access(
    state: &PrimaryAppState,
    team_id: i64,
    user_id: i64,
) -> Result<team::Model> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    if team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .is_none()
    {
        return Err(AsterError::auth_forbidden("not a member of this team"));
    }
    Ok(team)
}

pub(crate) async fn require_team_management_access(
    state: &PrimaryAppState,
    team_id: i64,
    user_id: i64,
) -> Result<team::Model> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    if !membership.role.can_manage_team() {
        return Err(AsterError::auth_forbidden(
            "team owner or admin role is required",
        ));
    }
    Ok(team)
}

pub(crate) async fn verify_folder_access(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    folder_id: i64,
) -> Result<folder::Model> {
    // 先校验当前 scope 还有效，再取实体做归属检查。
    // 这样所有调用方都能拿到“存在 + 属于当前空间 + 未进回收站”的 folder。
    require_scope_access(state, scope).await?;
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_active_folder_scope(&folder, scope)?;

    Ok(folder)
}

pub(crate) async fn verify_file_access(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<file::Model> {
    // 文件访问和文件夹访问保持同样语义：返回值一旦成功，就已经完成 scope
    // 校验和 trash 过滤，上层不需要再手写重复判断。
    require_scope_access(state, scope).await?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_active_file_scope(&file, scope)?;

    Ok(file)
}

pub(crate) async fn list_files_in_folder(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_by_folder(&state.db, user_id, folder_id).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_by_team_folder(&state.db, team_id, folder_id).await
        }
    }
}

pub(crate) async fn list_folders_in_parent(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_children(&state.db, user_id, parent_id).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_team_children(&state.db, team_id, parent_id).await
        }
    }
}
