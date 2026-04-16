use actix_web::HttpResponse;

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::workspace_storage_service::WorkspaceStorageScope;

use super::{
    DownloadDisposition, ensure_personal_file_scope, get_info_in_scope, if_none_match_matches,
    inline_sandbox_csp, requires_inline_sandbox,
};

pub(crate) async fn download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    tracing::debug!(
        scope = ?scope,
        file_id = id,
        has_if_none_match = if_none_match.is_some(),
        "starting file download"
    );
    let file = get_info_in_scope(state, scope, id).await?;
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    build_stream_response(state, &file, &blob, if_none_match).await
}

/// 下载文件（流式，不全量缓冲）
pub async fn download(
    state: &AppState,
    id: i64,
    user_id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    download_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        id,
        if_none_match,
    )
    .await
}

/// 下载文件（无用户校验，用于分享链接，流式）
pub async fn download_raw(
    state: &AppState,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    ensure_personal_file_scope(&f)?;
    download_raw_unchecked_with_file(state, f, if_none_match).await
}

async fn download_raw_unchecked_with_file(
    state: &AppState,
    f: file::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    build_stream_response(state, &f, &blob, if_none_match).await
}

/// 构建流式下载响应
pub(crate) async fn build_stream_response(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    build_stream_response_with_disposition(
        state,
        f,
        blob,
        DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

pub(crate) async fn build_stream_response_with_disposition(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    disposition: DownloadDisposition,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let requires_sandbox =
        disposition == DownloadDisposition::Inline && requires_inline_sandbox(&f.mime_type);

    if requires_sandbox {
        tracing::debug!(
            file_id = f.id,
            blob_id = blob.id,
            mime_type = %f.mime_type,
            "adding CSP sandbox for inline script-capable file"
        );
    }

    let etag = format!("\"{}\"", blob.hash);
    if let Some(if_none_match) = if_none_match
        && if_none_match_matches(if_none_match, &blob.hash)
    {
        tracing::debug!(
            file_id = f.id,
            blob_id = blob.id,
            disposition = ?disposition,
            "serving cached file response with 304"
        );
        let mut response = HttpResponse::NotModified();
        response.insert_header(("ETag", etag));
        response.insert_header(("Cache-Control", "private, max-age=0, must-revalidate"));
        if requires_sandbox {
            response.insert_header(("Content-Security-Policy", inline_sandbox_csp()));
            response.insert_header(("X-Content-Type-Options", "nosniff"));
        }
        return Ok(response.finish());
    }

    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let stream = driver.get_stream(&blob.storage_path).await?;

    // 64KB buffer — 比默认 4KB 减少系统调用和分配开销
    let reader_stream = tokio_util::io::ReaderStream::with_capacity(stream, 64 * 1024);

    tracing::debug!(
        file_id = f.id,
        blob_id = blob.id,
        policy_id = blob.policy_id,
        size = blob.size,
        disposition = ?disposition,
        "building streaming file response"
    );

    let mut response = HttpResponse::Ok();
    response.content_type(f.mime_type.clone());
    response.insert_header(("Content-Length", blob.size.to_string()));
    response.insert_header(("Content-Disposition", disposition.header_value(&f.name)));
    response.insert_header(("ETag", etag));
    response.insert_header(("Cache-Control", "private, max-age=0, must-revalidate"));
    if requires_sandbox {
        response.insert_header(("Content-Security-Policy", inline_sandbox_csp()));
        response.insert_header(("X-Content-Type-Options", "nosniff"));
    }
    // 跳过全局 Compress 中间件，避免压缩编码器缓冲导致内存暴涨
    response.insert_header(("Content-Encoding", "identity"));
    Ok(response.streaming(reader_stream))
}
