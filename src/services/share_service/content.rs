//! 分享服务子模块：`content`。

use crate::db::repository::{file_repo, share_repo};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{file_service, folder_service};

use super::shared::{
    load_share_file_resource, load_shared_folder_file_target, load_shared_subfolder_target,
    load_valid_folder_share_root, load_valid_share,
};

pub async fn download_shared_file(
    state: &AppState,
    token: &str,
    if_none_match: Option<&str>,
) -> Result<file_service::DownloadOutcome> {
    let share = load_valid_share(state, token).await?;
    let file = load_share_file_resource(state, &share).await?;
    download_share_resource_with_disposition(
        state,
        &share,
        &file,
        file_service::DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

pub async fn download_shared_folder_file(
    state: &AppState,
    token: &str,
    file_id: i64,
    if_none_match: Option<&str>,
) -> Result<file_service::DownloadOutcome> {
    let (share, file) = load_shared_folder_file_target(state, token, file_id).await?;
    download_share_resource_with_disposition(
        state,
        &share,
        &file,
        file_service::DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn list_shared_folder(
    state: &AppState,
    token: &str,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<folder_service::FolderContents> {
    let (_, folder_id) = load_valid_folder_share_root(state, token).await?;
    tracing::debug!(
        folder_id,
        folder_limit,
        folder_offset,
        file_limit,
        has_file_cursor = file_cursor.is_some(),
        sort_by = ?sort_by,
        sort_order = ?sort_order,
        "listing shared folder root"
    );

    let contents = folder_service::list_shared(
        state,
        folder_id,
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
        sort_by,
        sort_order,
    )
    .await?;
    tracing::debug!(
        folder_id,
        folders_total = contents.folders_total,
        files_total = contents.files_total,
        returned_folders = contents.folders.len(),
        returned_files = contents.files.len(),
        "listed shared folder root"
    );
    Ok(contents)
}

pub async fn get_shared_thumbnail(
    state: &AppState,
    token: &str,
) -> Result<file_service::ThumbnailResult> {
    let share = load_valid_share(state, token).await?;
    tracing::debug!(share_id = share.id, "loading shared thumbnail");
    let file = load_share_file_resource(state, &share).await?;
    crate::services::thumbnail_service::ensure_supported_mime(&file.mime_type)?;

    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    let data = crate::services::thumbnail_service::get_or_generate(state, &blob).await?;
    let thumbnail_version =
        crate::services::thumbnail_service::thumbnail_version(&blob).to_string();
    tracing::debug!(
        share_id = share.id,
        file_id = file.id,
        blob_id = blob.id,
        "loaded shared thumbnail"
    );
    Ok(file_service::ThumbnailResult {
        data,
        blob_hash: blob.hash,
        thumbnail_version: Some(thumbnail_version),
    })
}

pub async fn get_shared_folder_file_thumbnail(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<file_service::ThumbnailResult> {
    let (_, file) = load_shared_folder_file_target(state, token, file_id).await?;
    tracing::debug!(file_id = file.id, "loading shared folder file thumbnail");

    crate::services::thumbnail_service::ensure_supported_mime(&file.mime_type)?;

    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    let data = crate::services::thumbnail_service::get_or_generate(state, &blob).await?;
    let thumbnail_version =
        crate::services::thumbnail_service::thumbnail_version(&blob).to_string();
    tracing::debug!(
        file_id = file.id,
        blob_id = blob.id,
        "loaded shared folder file thumbnail"
    );
    Ok(file_service::ThumbnailResult {
        data,
        blob_hash: blob.hash,
        thumbnail_version: Some(thumbnail_version),
    })
}

pub(crate) async fn load_preview_shared_file(
    state: &AppState,
    token: &str,
) -> Result<(share::Model, crate::entities::file::Model)> {
    let share = load_valid_share(state, token).await?;
    let file = load_share_file_resource(state, &share).await?;
    Ok((share, file))
}

pub(crate) async fn load_preview_shared_folder_file(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<(share::Model, crate::entities::file::Model)> {
    load_shared_folder_file_target(state, token, file_id).await
}

#[allow(clippy::too_many_arguments)]
pub async fn list_shared_subfolder(
    state: &AppState,
    token: &str,
    folder_id: i64,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<folder_service::FolderContents> {
    let (_, target) = load_shared_subfolder_target(state, token, folder_id).await?;
    tracing::debug!(
        folder_id = target.id,
        folder_limit,
        folder_offset,
        file_limit,
        has_file_cursor = file_cursor.is_some(),
        sort_by = ?sort_by,
        sort_order = ?sort_order,
        "listing shared subfolder"
    );

    let contents = folder_service::list_shared(
        state,
        target.id,
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
        sort_by,
        sort_order,
    )
    .await?;
    tracing::debug!(
        folder_id = target.id,
        folders_total = contents.folders_total,
        files_total = contents.files_total,
        returned_folders = contents.folders.len(),
        returned_files = contents.files.len(),
        "listed shared subfolder"
    );
    Ok(contents)
}

async fn download_share_resource_with_disposition(
    state: &AppState,
    share: &share::Model,
    file: &crate::entities::file::Model,
    disposition: file_service::DownloadDisposition,
    if_none_match: Option<&str>,
) -> Result<file_service::DownloadOutcome> {
    tracing::debug!(
        share_id = share.id,
        file_id = file.id,
        disposition = ?disposition,
        has_if_none_match = if_none_match.is_some(),
        "starting shared file download"
    );
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;

    if let Some(if_none_match) = if_none_match
        && file_service::if_none_match_matches(if_none_match, &blob.hash)
    {
        tracing::debug!(
            share_id = share.id,
            file_id = file.id,
            "shared file download satisfied by ETag"
        );
        return file_service::build_download_outcome_with_disposition(
            state,
            file,
            &blob,
            disposition,
            Some(if_none_match),
        )
        .await;
    }

    match share_repo::increment_download_count(&state.db, share.id).await {
        Ok(true) => {}
        Ok(false) => {
            return Err(AsterError::share_download_limit("download limit reached"));
        }
        Err(error) => {
            tracing::warn!(
                share_id = share.id,
                "failed to increment download count: {error}"
            );
            return Err(error);
        }
    }

    match file_service::build_download_outcome_with_disposition(
        state,
        file,
        &blob,
        disposition,
        None,
    )
    .await
    {
        Ok(mut outcome) => {
            // 如果是流式响应，挂一个 abort hook：客户端中途断连导致 body 未读到 EOF 就 drop 时，
            // 回滚刚才的 increment，避免 `download_count` 虚增、提前触碰 `max_downloads`。
            // NotModified/PresignedRedirect 一次性响应不需要挂 hook。
            if let file_service::DownloadOutcome::Stream(ref mut s) = outcome {
                let db = state.db.clone();
                let share_id = share.id;
                s.on_abort = Some(Box::new(move || {
                    tokio::spawn(async move {
                        if let Err(e) = share_repo::decrement_download_count(&db, share_id).await {
                            tracing::warn!(
                                share_id,
                                "failed to roll back download count on client abort: {e}"
                            );
                        }
                    });
                }));
            }
            tracing::debug!(
                share_id = share.id,
                file_id = file.id,
                "completed shared file download"
            );
            Ok(outcome)
        }
        Err(error) => {
            match share_repo::decrement_download_count(&state.db, share.id).await {
                Ok(true) => {}
                Ok(false) => {
                    tracing::warn!(
                        share_id = share.id,
                        "failed to roll back download count after response build failure"
                    );
                }
                Err(rollback_error) => {
                    tracing::warn!(
                        share_id = share.id,
                        "failed to roll back download count after response build failure: {rollback_error}"
                    );
                }
            }
            Err(error)
        }
    }
}
