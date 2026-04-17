use crate::db::repository::file_repo;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    task_service, thumbnail_service, workspace_storage_service::WorkspaceStorageScope,
};

use super::get_info_in_scope;

/// 缩略图查询结果：有数据直接返回，正在生成则标记 pending
pub struct ThumbnailResult {
    pub data: Vec<u8>,
    pub blob_hash: String,
    pub thumbnail_version: Option<String>,
}

pub(crate) async fn get_thumbnail_data_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<Option<ThumbnailResult>> {
    let f = get_info_in_scope(state, scope, file_id).await?;
    thumbnail_service::ensure_supported_mime(&f.mime_type)?;
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    match thumbnail_service::load_thumbnail_if_exists(state, &blob).await? {
        Some(data) => {
            let thumbnail_version = thumbnail_service::thumbnail_version(&blob).to_string();
            Ok(Some(ThumbnailResult {
                data,
                blob_hash: blob.hash,
                thumbnail_version: Some(thumbnail_version),
            }))
        }
        None => {
            task_service::ensure_thumbnail_task(state, &blob, &f.mime_type).await?;
            Ok(None)
        }
    }
}

/// 获取文件缩略图。返回 `Ok(Some(data))` 直接有图；`Ok(None)` 表示正在后台生成。
pub async fn get_thumbnail_data(
    state: &AppState,
    file_id: i64,
    user_id: i64,
) -> Result<Option<ThumbnailResult>> {
    get_thumbnail_data_in_scope(state, WorkspaceStorageScope::Personal { user_id }, file_id).await
}
