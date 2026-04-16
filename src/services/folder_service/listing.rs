use crate::db::repository::{file_repo, folder_repo, share_repo};
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};

use super::{
    FileCursor, FolderContents, build_file_list_items, build_folder_list_items,
    ensure_personal_folder_scope,
};

#[allow(clippy::too_many_arguments)]
async fn build_folder_contents(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folders: Vec<crate::entities::folder::Model>,
    folders_total: u64,
    files: Vec<crate::entities::file::Model>,
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
        WorkspaceStorageScope::Personal { user_id } => tokio::try_join!(
            share_repo::find_active_file_ids(&state.db, user_id, &file_ids),
            share_repo::find_active_folder_ids(&state.db, user_id, &folder_ids),
        )?,
        WorkspaceStorageScope::Team { team_id, .. } => tokio::try_join!(
            share_repo::find_active_team_file_ids(&state.db, team_id, &file_ids),
            share_repo::find_active_team_folder_ids(&state.db, team_id, &folder_ids),
        )?,
    };

    Ok(FolderContents {
        folders: build_folder_list_items(folders, &shared_folder_ids),
        files: build_file_list_items(files, &shared_file_ids),
        folders_total,
        files_total,
        next_file_cursor,
    })
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
    tracing::debug!(
        scope = ?scope,
        parent_id,
        folder_limit,
        folder_offset,
        file_limit,
        has_file_cursor = file_cursor.is_some(),
        sort_by = ?sort_by,
        sort_order = ?sort_order,
        "listing folder contents"
    );
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
            let folder_task = async {
                if folder_limit == 0 {
                    Ok((
                        vec![],
                        folder_repo::find_children_paginated(
                            &state.db, user_id, parent_id, 0, 0, sort_by, sort_order,
                        )
                        .await?
                        .1,
                    ))
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
                    .await
                }
            };
            let file_task = async {
                if file_limit == 0 {
                    Ok((
                        vec![],
                        file_repo::find_by_folder_cursor(
                            &state.db, user_id, parent_id, 0, None, sort_by, sort_order,
                        )
                        .await?
                        .1,
                    ))
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
                    .await
                }
            };
            let ((folders, folders_total), (files, files_total)) =
                tokio::try_join!(folder_task, file_task)?;

            (folders, folders_total, files, files_total)
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            let folder_task = async {
                if folder_limit == 0 {
                    Ok((
                        vec![],
                        folder_repo::find_team_children_paginated(
                            &state.db, team_id, parent_id, 0, 0, sort_by, sort_order,
                        )
                        .await?
                        .1,
                    ))
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
                    .await
                }
            };
            let file_task = async {
                if file_limit == 0 {
                    Ok((
                        vec![],
                        file_repo::find_by_team_folder_cursor(
                            &state.db, team_id, parent_id, 0, None, sort_by, sort_order,
                        )
                        .await?
                        .1,
                    ))
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
                    .await
                }
            };
            let ((folders, folders_total), (files, files_total)) =
                tokio::try_join!(folder_task, file_task)?;

            (folders, folders_total, files, files_total)
        }
    };

    let contents = build_folder_contents(
        state,
        scope,
        folders,
        folders_total,
        files,
        files_total,
        sort_by,
        file_limit,
    )
    .await?;
    tracing::debug!(
        scope = ?scope,
        parent_id,
        folders_total = contents.folders_total,
        files_total = contents.files_total,
        returned_folders = contents.folders.len(),
        returned_files = contents.files.len(),
        has_next_file_cursor = contents.next_file_cursor.is_some(),
        "listed folder contents"
    );
    Ok(contents)
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
    tracing::debug!(
        folder_id,
        folder_limit,
        folder_offset,
        file_limit,
        has_file_cursor = file_cursor.is_some(),
        sort_by = ?sort_by,
        sort_order = ?sort_order,
        "listing shared folder contents"
    );
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    let contents = if let Some(team_id) = folder.team_id {
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

        build_folder_contents(
            state,
            WorkspaceStorageScope::Team {
                team_id,
                actor_user_id: folder.user_id,
            },
            folders,
            folders_total,
            files,
            files_total,
            sort_by,
            file_limit,
        )
        .await?
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
        .await?
    };
    tracing::debug!(
        folder_id,
        folders_total = contents.folders_total,
        files_total = contents.files_total,
        returned_folders = contents.folders.len(),
        returned_files = contents.files.len(),
        has_next_file_cursor = contents.next_file_cursor.is_some(),
        "listed shared folder contents"
    );
    Ok(contents)
}
