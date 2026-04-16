use std::collections::HashMap;

use chrono::Utc;
use sea_orm::DatabaseConnection;

use crate::db::repository::{file_repo, folder_repo, share_repo, team_repo};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    file_service, folder_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::EntityType;

use super::models::{ShareStatus, ShareTarget, share_target_for_share};

pub(super) fn validate_max_downloads(max_downloads: i64) -> Result<()> {
    if max_downloads < 0 {
        return Err(AsterError::validation_error(
            "max_downloads cannot be negative",
        ));
    }
    Ok(())
}

fn ensure_share_scope(share: &share::Model, scope: WorkspaceStorageScope) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            if share.team_id.is_some() {
                return Err(AsterError::auth_forbidden(
                    "share belongs to a team workspace",
                ));
            }
            crate::utils::verify_owner(share.user_id, user_id, "share")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if share.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden(
                    "share is outside team workspace",
                ));
            }
        }
    }

    Ok(())
}

pub(super) async fn lock_share_resource_in_scope<C: sea_orm::ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    file_id: Option<i64>,
    folder_id: Option<i64>,
) -> Result<()> {
    if let Some(file_id) = file_id {
        let file = file_repo::lock_by_id(db, file_id).await?;
        workspace_storage_service::ensure_active_file_scope(&file, scope)?;
    }

    if let Some(folder_id) = folder_id {
        let folder = folder_repo::lock_by_id(db, folder_id).await?;
        workspace_storage_service::ensure_active_folder_scope(&folder, scope)?;
    }

    Ok(())
}

pub(super) async fn load_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
) -> Result<share::Model> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let share = share_repo::find_by_id(&state.db, share_id).await?;
    ensure_share_scope(&share, scope)?;
    Ok(share)
}

pub(super) async fn load_valid_share(state: &AppState, token: &str) -> Result<share::Model> {
    let share = load_share_record(state, token).await?;
    validate_share(&share)?;
    Ok(share)
}

pub(super) async fn load_share_record(state: &AppState, token: &str) -> Result<share::Model> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;
    if let Some(team_id) = share.team_id {
        match team_repo::find_active_by_id(&state.db, team_id).await {
            Ok(_) => {}
            Err(AsterError::RecordNotFound(_)) => {
                return Err(AsterError::share_not_found(format!("token={token}")));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(share)
}

pub(super) fn ensure_share_matches_file(
    share: &share::Model,
    file: &crate::entities::file::Model,
) -> Result<()> {
    if let Some(team_id) = share.team_id {
        if file.team_id != Some(team_id) {
            return Err(AsterError::auth_forbidden("file is outside shared scope"));
        }
    } else {
        file_service::ensure_personal_file_scope(file)?;
        crate::utils::verify_owner(file.user_id, share.user_id, "file")?;
    }
    Ok(())
}

pub(super) fn ensure_share_matches_folder(
    share: &share::Model,
    folder: &crate::entities::folder::Model,
) -> Result<()> {
    if let Some(team_id) = share.team_id {
        if folder.team_id != Some(team_id) {
            return Err(AsterError::auth_forbidden("folder is outside shared scope"));
        }
    } else {
        folder_service::ensure_personal_folder_scope(folder)?;
        crate::utils::verify_owner(folder.user_id, share.user_id, "folder")?;
    }
    Ok(())
}

pub(super) async fn load_share_file_resource(
    state: &AppState,
    share: &share::Model,
) -> Result<crate::entities::file::Model> {
    let file_id = match share_target_for_share(share)? {
        ShareTarget {
            r#type: EntityType::File,
            id,
        } => id,
        ShareTarget {
            r#type: EntityType::Folder,
            ..
        } => {
            return Err(AsterError::validation_error(
                "this share is for a folder, not a file",
            ));
        }
    };
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_share_matches_file(share, &file)?;
    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }
    Ok(file)
}

pub(super) async fn load_share_folder_resource(
    state: &AppState,
    share: &share::Model,
) -> Result<crate::entities::folder::Model> {
    let folder_id = match share_target_for_share(share)? {
        ShareTarget {
            r#type: EntityType::Folder,
            id,
        } => id,
        ShareTarget {
            r#type: EntityType::File,
            ..
        } => {
            return Err(AsterError::validation_error(
                "this share is for a file, not a folder",
            ));
        }
    };
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_share_matches_folder(share, &folder)?;
    if folder.deleted_at.is_some() {
        return Err(AsterError::folder_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    Ok(folder)
}

pub(super) async fn load_valid_folder_share_root(
    state: &AppState,
    token: &str,
) -> Result<(share::Model, i64)> {
    let share = load_valid_share(state, token).await?;
    let root = load_share_folder_resource(state, &share).await?;
    Ok((share, root.id))
}

pub(super) async fn load_shared_folder_file_target(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<(share::Model, crate::entities::file::Model)> {
    let (share, root_folder_id) = load_valid_folder_share_root(state, token).await?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_share_matches_file(&share, &file)?;
    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }
    let file_folder_id = file
        .folder_id
        .ok_or_else(|| AsterError::auth_forbidden("file is outside shared folder scope"))?;
    folder_service::verify_folder_in_scope(&state.db, file_folder_id, root_folder_id).await?;
    Ok((share, file))
}

pub(super) async fn load_shared_subfolder_target(
    state: &AppState,
    token: &str,
    folder_id: i64,
) -> Result<(share::Model, crate::entities::folder::Model)> {
    let (share, root_folder_id) = load_valid_folder_share_root(state, token).await?;
    let target = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_share_matches_folder(&share, &target)?;
    if target.deleted_at.is_some() {
        return Err(AsterError::folder_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    folder_service::verify_folder_in_scope(&state.db, folder_id, root_folder_id).await?;
    Ok((share, target))
}

pub(super) fn validate_share(share: &share::Model) -> Result<()> {
    share_target_for_share(share)?;

    if let Some(exp) = share.expires_at
        && exp < Utc::now()
    {
        return Err(AsterError::share_expired("share has expired"));
    }
    validate_share_download_limit(share)?;
    Ok(())
}

fn validate_share_download_limit(share: &share::Model) -> Result<()> {
    if share.max_downloads > 0 && share.download_count >= share.max_downloads {
        return Err(AsterError::share_download_limit("download limit reached"));
    }

    Ok(())
}

pub(super) fn resolve_share_resource(
    share: &share::Model,
    file_map: &HashMap<i64, crate::entities::file::Model>,
    folder_map: &HashMap<i64, crate::entities::folder::Model>,
) -> Result<(i64, String, EntityType, bool)> {
    match share_target_for_share(share)? {
        ShareTarget {
            r#type: EntityType::File,
            id: file_id,
        } => {
            if let Some(file) = file_map.get(&file_id) {
                return Ok((
                    file_id,
                    file.name.clone(),
                    EntityType::File,
                    file.deleted_at.is_some(),
                ));
            }
            Ok((file_id, "Unknown file".to_string(), EntityType::File, true))
        }
        ShareTarget {
            r#type: EntityType::Folder,
            id: folder_id,
        } => {
            if let Some(folder) = folder_map.get(&folder_id) {
                return Ok((
                    folder_id,
                    folder.name.clone(),
                    EntityType::Folder,
                    folder.deleted_at.is_some(),
                ));
            }
            Ok((
                folder_id,
                "Unknown folder".to_string(),
                EntityType::Folder,
                true,
            ))
        }
    }
}

pub(super) fn resolve_share_status(share: &share::Model, resource_deleted: bool) -> ShareStatus {
    if resource_deleted {
        return ShareStatus::Deleted;
    }
    if share
        .expires_at
        .is_some_and(|expires_at| expires_at < Utc::now())
    {
        return ShareStatus::Expired;
    }
    if share.max_downloads > 0 && share.download_count >= share.max_downloads {
        return ShareStatus::Exhausted;
    }
    ShareStatus::Active
}

pub(super) fn remaining_downloads(max_downloads: i64, download_count: i64) -> Option<i64> {
    (max_downloads > 0).then_some((max_downloads - download_count).max(0))
}

pub(super) async fn resolve_share_name(
    db: &DatabaseConnection,
    share: &share::Model,
) -> Result<(String, String, Option<String>, Option<i64>)> {
    match share_target_for_share(share)? {
        ShareTarget {
            r#type: EntityType::File,
            id: file_id,
        } => {
            let file = file_repo::find_by_id(db, file_id).await?;
            ensure_share_matches_file(share, &file)?;
            if file.deleted_at.is_some() {
                return Err(AsterError::file_not_found(format!(
                    "file #{file_id} is in trash"
                )));
            }
            Ok((
                file.name,
                "file".to_string(),
                Some(file.mime_type),
                Some(file.size),
            ))
        }
        ShareTarget {
            r#type: EntityType::Folder,
            id: folder_id,
        } => {
            let folder = folder_repo::find_by_id(db, folder_id).await?;
            ensure_share_matches_folder(share, &folder)?;
            if folder.deleted_at.is_some() {
                return Err(AsterError::folder_not_found(format!(
                    "folder #{folder_id} is in trash"
                )));
            }
            Ok((folder.name, "folder".to_string(), None, None))
        }
    }
}
