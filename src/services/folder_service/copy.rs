use chrono::Utc;
use sea_orm::Set;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::folder;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    storage_change_service,
    workspace_models::FolderInfo,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

use super::ensure_folder_model_in_scope;

const MAX_COPY_NAME_RETRIES: usize = 32;

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
    tracing::debug!(
        scope = ?scope,
        src_folder_id = src_id,
        dest_parent_id,
        "copying folder tree"
    );
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
    for _ in 0..MAX_COPY_NAME_RETRIES {
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

        if exists {
            dest_name = crate::utils::next_copy_name(&dest_name);
            continue;
        }

        match recursive_copy_folder_in_scope(state, scope, src_id, dest_parent_id, &dest_name).await
        {
            Ok(copied) => {
                storage_change_service::publish(
                    state,
                    storage_change_service::StorageChangeEvent::new(
                        storage_change_service::StorageChangeKind::FolderCreated,
                        scope,
                        vec![],
                        vec![copied.id],
                        vec![copied.parent_id],
                    ),
                );
                tracing::debug!(
                    scope = ?scope,
                    src_folder_id = src_id,
                    copied_folder_id = copied.id,
                    parent_id = copied.parent_id,
                    name = %copied.name,
                    "copied folder tree"
                );
                return Ok(copied);
            }
            Err(err) if folder_repo::is_duplicate_name_error(&err, &dest_name) => {
                dest_name = crate::utils::next_copy_name(&dest_name);
            }
            Err(err) => return Err(err),
        }
    }

    Err(AsterError::validation_error(format!(
        "failed to allocate a unique copy name for '{}'",
        src.name
    )))
}

/// 复制文件夹（递归复制所有文件和子文件夹）
///
/// `dest_parent_id = None` 表示复制到根目录。
pub async fn copy_folder(
    state: &AppState,
    src_id: i64,
    user_id: i64,
    dest_parent_id: Option<i64>,
) -> Result<FolderInfo> {
    copy_folder_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        src_id,
        dest_parent_id,
    )
    .await
    .map(Into::into)
}
