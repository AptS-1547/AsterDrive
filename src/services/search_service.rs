use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

use crate::db::repository::{search_repo, share_repo};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    folder_service::{FileListItem, FolderListItem, build_folder_list_items},
    workspace_storage_service::{self, WorkspaceStorageScope},
};

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SearchParams {
    /// Name search pattern (case-insensitive substring match)
    pub q: Option<String>,
    /// Result type filter: "file", "folder", or "all" (default)
    #[serde(rename = "type")]
    pub search_type: Option<String>,
    /// Filter by exact MIME type (e.g. "image/png")
    pub mime_type: Option<String>,
    /// Minimum file size in bytes
    pub min_size: Option<i64>,
    /// Maximum file size in bytes
    pub max_size: Option<i64>,
    /// ISO 8601 datetime — only return items created after this time
    pub created_after: Option<String>,
    /// ISO 8601 datetime — only return items created before this time
    pub created_before: Option<String>,
    /// Scope search to a specific folder (folder_id for files, parent_id for folders)
    pub folder_id: Option<i64>,
    /// Max results per type (default 50, max 100)
    pub limit: Option<u64>,
    /// Offset for pagination
    pub offset: Option<u64>,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SearchResults {
    pub files: Vec<FileListItem>,
    pub folders: Vec<FolderListItem>,
    pub total_files: u64,
    pub total_folders: u64,
}

type SearchDateRange = (
    Option<chrono::DateTime<chrono::Utc>>,
    Option<chrono::DateTime<chrono::Utc>>,
);

fn build_search_file_list_items(
    files: Vec<search_repo::FileSearchItem>,
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

fn validate_search_params(params: &SearchParams) -> Result<()> {
    if let Some(ref q) = params.q
        && q.trim().is_empty()
    {
        return Err(AsterError::validation_error(
            "search query must not be empty",
        ));
    }

    if let Some(search_type) = params.search_type.as_deref()
        && !matches!(search_type, "file" | "folder" | "all")
    {
        return Err(AsterError::validation_error(
            "type must be one of: file, folder, all",
        ));
    }

    if let (Some(min), Some(max)) = (params.min_size, params.max_size)
        && min > max
    {
        return Err(AsterError::validation_error("min_size must be <= max_size"));
    }

    Ok(())
}

fn parse_search_dates(params: &SearchParams) -> Result<SearchDateRange> {
    let parse_field =
        |field: &str, value: Option<&str>| -> Result<Option<chrono::DateTime<chrono::Utc>>> {
            value
                .map(|raw| {
                    DateTime::parse_from_rfc3339(raw)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|_| {
                            AsterError::validation_error(format!(
                                "{field} must be a valid RFC3339 datetime"
                            ))
                        })
                })
                .transpose()
        };

    let created_after = parse_field("created_after", params.created_after.as_deref())?;
    let created_before = parse_field("created_before", params.created_before.as_deref())?;

    if let (Some(after), Some(before)) = (created_after, created_before)
        && after > before
    {
        return Err(AsterError::validation_error(
            "created_after must be <= created_before",
        ));
    }

    Ok((created_after, created_before))
}

pub(crate) async fn search_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    params: &SearchParams,
) -> Result<SearchResults> {
    validate_search_params(params)?;
    workspace_storage_service::require_scope_access(state, scope).await?;

    let limit = params.limit.unwrap_or(50).clamp(1, 100);
    let offset = params.offset.unwrap_or(0);

    let search_type = params.search_type.as_deref().unwrap_or("all");
    let (created_after, created_before) = parse_search_dates(params)?;

    let (files, total_files, folders, total_folders, shared_file_ids, shared_folder_ids) =
        match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                let (files, total_files) = if search_type == "folder" {
                    (vec![], 0)
                } else {
                    search_repo::search_files(
                        &state.db,
                        user_id,
                        params.q.as_deref(),
                        params.mime_type.as_deref(),
                        params.min_size,
                        params.max_size,
                        created_after,
                        created_before,
                        params.folder_id,
                        limit,
                        offset,
                    )
                    .await?
                };

                let (folders, total_folders) = if search_type == "file" {
                    (vec![], 0)
                } else {
                    search_repo::search_folders(
                        &state.db,
                        user_id,
                        params.q.as_deref(),
                        created_after,
                        created_before,
                        params.folder_id,
                        limit,
                        offset,
                    )
                    .await?
                };

                let file_ids: Vec<i64> = files.iter().map(|file| file.id).collect();
                let folder_ids: Vec<i64> = folders.iter().map(|folder| folder.id).collect();
                let shared_file_ids =
                    share_repo::find_active_file_ids(&state.db, user_id, &file_ids).await?;
                let shared_folder_ids =
                    share_repo::find_active_folder_ids(&state.db, user_id, &folder_ids).await?;

                (
                    files,
                    total_files,
                    folders,
                    total_folders,
                    shared_file_ids,
                    shared_folder_ids,
                )
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                let (files, total_files) = if search_type == "folder" {
                    (vec![], 0)
                } else {
                    search_repo::search_team_files(
                        &state.db,
                        team_id,
                        params.q.as_deref(),
                        params.mime_type.as_deref(),
                        params.min_size,
                        params.max_size,
                        created_after,
                        created_before,
                        params.folder_id,
                        limit,
                        offset,
                    )
                    .await?
                };

                let (folders, total_folders) = if search_type == "file" {
                    (vec![], 0)
                } else {
                    search_repo::search_team_folders(
                        &state.db,
                        team_id,
                        params.q.as_deref(),
                        created_after,
                        created_before,
                        params.folder_id,
                        limit,
                        offset,
                    )
                    .await?
                };

                let file_ids: Vec<i64> = files.iter().map(|file| file.id).collect();
                let folder_ids: Vec<i64> = folders.iter().map(|folder| folder.id).collect();
                let shared_file_ids =
                    share_repo::find_active_team_file_ids(&state.db, team_id, &file_ids).await?;
                let shared_folder_ids =
                    share_repo::find_active_team_folder_ids(&state.db, team_id, &folder_ids)
                        .await?;

                (
                    files,
                    total_files,
                    folders,
                    total_folders,
                    shared_file_ids,
                    shared_folder_ids,
                )
            }
        };

    Ok(SearchResults {
        files: build_search_file_list_items(files, &shared_file_ids),
        folders: build_folder_list_items(folders, &shared_folder_ids),
        total_files,
        total_folders,
    })
}

pub async fn search(
    state: &AppState,
    user_id: i64,
    params: &SearchParams,
) -> Result<SearchResults> {
    search_in_scope(state, WorkspaceStorageScope::Personal { user_id }, params).await
}

pub async fn search_in_team(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    params: &SearchParams,
) -> Result<SearchResults> {
    search_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        params,
    )
    .await
}
