use chrono::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::db::repository::search_repo::{self, FileSearchItem};
use crate::entities::folder;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

#[derive(Deserialize, IntoParams, ToSchema)]
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

#[derive(Serialize, ToSchema)]
pub struct SearchResults {
    pub files: Vec<FileSearchItem>,
    pub folders: Vec<folder::Model>,
    pub total_files: u64,
    pub total_folders: u64,
}

pub async fn search(
    state: &AppState,
    user_id: i64,
    params: &SearchParams,
) -> Result<SearchResults> {
    // Validation
    if let Some(ref q) = params.q {
        if q.is_empty() {
            return Err(AsterError::validation_error(
                "search query must not be empty",
            ));
        }
    }

    if let (Some(min), Some(max)) = (params.min_size, params.max_size) {
        if min > max {
            return Err(AsterError::validation_error("min_size must be <= max_size"));
        }
    }

    let limit = params.limit.unwrap_or(50).clamp(1, 100);
    let offset = params.offset.unwrap_or(0);

    let search_type = params.search_type.as_deref().unwrap_or("all");

    // Parse ISO 8601 dates (silently ignore malformed values)
    let created_after = params
        .created_after
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let created_before = params
        .created_before
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

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

    Ok(SearchResults {
        files,
        folders,
        total_files,
        total_folders,
    })
}
