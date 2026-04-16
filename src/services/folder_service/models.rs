use serde::Serialize;
use std::collections::HashSet;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::entities::{file, folder};

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderAncestorItem {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FileListItem {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub mime_type: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderListItem {
    pub id: i64,
    pub name: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub is_locked: bool,
    pub is_shared: bool,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FileCursor {
    /// 排序字段值（序列化为字符串）
    pub value: String,
    pub id: i64,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FolderContents {
    pub folders: Vec<FolderListItem>,
    pub files: Vec<FileListItem>,
    pub folders_total: u64,
    pub files_total: u64,
    /// 下一页 cursor，None 表示已到最后一页
    pub next_file_cursor: Option<FileCursor>,
}

pub fn build_file_list_items(
    files: Vec<file::Model>,
    shared_file_ids: &HashSet<i64>,
) -> Vec<FileListItem> {
    files
        .into_iter()
        .map(|file| FileListItem {
            id: file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            updated_at: file.updated_at,
            is_locked: file.is_locked,
            is_shared: shared_file_ids.contains(&file.id),
        })
        .collect()
}

pub fn build_folder_list_items(
    folders: Vec<folder::Model>,
    shared_folder_ids: &HashSet<i64>,
) -> Vec<FolderListItem> {
    folders
        .into_iter()
        .map(|folder| FolderListItem {
            id: folder.id,
            name: folder.name,
            updated_at: folder.updated_at,
            is_locked: folder.is_locked,
            is_shared: shared_folder_ids.contains(&folder.id),
        })
        .collect()
}
