//! 文件服务子模块：`thumbnail`。

use crate::db::repository::file_repo;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::{
    media_processing_service, task_service, workspace_storage_service::WorkspaceStorageScope,
};

use super::get_info_in_scope;

/// 缩略图查询结果：有数据直接返回，正在生成则标记 pending
pub struct ThumbnailResult {
    pub data: Vec<u8>,
    pub blob_hash: String,
    pub thumbnail_version: Option<String>,
}

pub(crate) async fn get_thumbnail_data_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<Option<ThumbnailResult>> {
    let f = get_info_in_scope(state, scope, file_id).await?;
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    let thumbnail = match media_processing_service::load_thumbnail_if_exists(
        state,
        &blob,
        &f.name,
        &f.mime_type,
    )
    .await
    {
        Ok(thumbnail) => thumbnail,
        Err(AsterError::PreconditionFailed(message)) => {
            return Err(AsterError::validation_error(message));
        }
        Err(error) => return Err(error),
    };

    match thumbnail {
        Some(thumbnail) => Ok(Some(ThumbnailResult {
            data: thumbnail.data,
            blob_hash: blob.hash,
            thumbnail_version: Some(thumbnail.thumbnail_version),
        })),
        None => {
            match task_service::ensure_thumbnail_task(state, &blob, &f.name, &f.mime_type).await {
                Ok(()) => {}
                Err(AsterError::PreconditionFailed(message)) => {
                    return Err(AsterError::validation_error(message));
                }
                Err(error) => return Err(error),
            }
            Ok(None)
        }
    }
}

/// 获取文件缩略图。返回 `Ok(Some(data))` 直接有图；`Ok(None)` 表示正在后台生成。
pub async fn get_thumbnail_data(
    state: &PrimaryAppState,
    file_id: i64,
    user_id: i64,
) -> Result<Option<ThumbnailResult>> {
    get_thumbnail_data_in_scope(state, WorkspaceStorageScope::Personal { user_id }, file_id).await
}
