use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// Batch delete files and folders.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchDeleteReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
}

/// Batch move files and folders to a target folder.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchMoveReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// Target folder ID (`None` = root directory).
    pub target_folder_id: Option<i64>,
}

/// Batch copy files and folders to a target folder.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchCopyReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// Target folder ID (`None` = root directory).
    pub target_folder_id: Option<i64>,
}

/// Request an archive download ticket for the selected files and folders.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveDownloadReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
}

/// Request an archive compression task for the selected files and folders.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveCompressReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
    pub target_folder_id: Option<i64>,
}
