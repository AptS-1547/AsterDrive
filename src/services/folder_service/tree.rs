use std::collections::HashSet;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::Result;
use crate::services::workspace_storage_service::WorkspaceStorageScope;

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
