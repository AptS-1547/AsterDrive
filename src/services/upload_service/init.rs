//! 上传初始化阶段。
//!
//! 这里不真正写入文件内容，只负责：
//! - 解析目标路径和目录自动创建
//! - 解析存储策略与大小限制
//! - 协商最终上传模式
//! - 在需要 session 的模式下预先写入 upload_sessions

mod context;
mod s3;

use chrono::{Duration, Utc};

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::upload_service::scope::{personal_scope, team_scope};
use crate::services::upload_service::shared::generate_upload_id;
use crate::services::workspace_storage_service::WorkspaceStorageScope;
use crate::types::{UploadMode, UploadSessionStatus};
use crate::utils::{numbers, paths};

use self::context::{
    InitUploadContext, UploadSessionRecordParams, direct_upload_response, persist_upload_session,
    resolve_init_upload_context, upload_fits_single_request,
};

async fn init_upload_for_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    tracing::debug!(
        scope = ?scope,
        folder_id,
        filename = %filename,
        total_size,
        relative_path = relative_path.unwrap_or(""),
        "initializing upload session"
    );

    let ctx =
        resolve_init_upload_context(state, scope, filename, total_size, folder_id, relative_path)
            .await?;

    if let Some(response) = s3::init_s3_upload(state, &ctx).await? {
        return Ok(response);
    }

    // 非 S3 或未启用 multipart 时，小文件直接走 direct upload，不需要 upload session。
    if upload_fits_single_request(ctx.total_size, ctx.policy.chunk_size) {
        tracing::debug!(
            scope = ?ctx.scope,
            policy_id = ctx.policy.id,
            mode = ?UploadMode::Direct,
            folder_id = ctx.target.folder_id,
            "selected direct upload mode"
        );
        return Ok(direct_upload_response());
    }

    init_chunked_upload_session(state, &ctx).await
}

async fn init_chunked_upload_session(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<InitUploadResponse> {
    // 本地 / 其他非 direct 场景：服务端维护分片目录与 upload session，
    // complete 阶段会把这些 chunk 组装成最终文件。
    let chunk_size = ctx.policy.chunk_size;
    let total_chunks = numbers::calc_total_chunks(ctx.total_size, chunk_size, "chunked upload")?;
    let upload_id = generate_upload_id(&state.db).await?;

    prepare_chunked_upload_temp_dir(state, &upload_id).await?;
    persist_upload_session(
        &state.db,
        UploadSessionRecordParams {
            upload_id: upload_id.clone(),
            scope: ctx.scope,
            filename: ctx.target.filename.clone(),
            total_size: ctx.total_size,
            chunk_size,
            total_chunks,
            folder_id: ctx.target.folder_id,
            policy_id: ctx.policy.id,
            status: UploadSessionStatus::Uploading,
            s3_temp_key: None,
            s3_multipart_id: None,
            expires_at: Utc::now() + Duration::hours(24),
        },
    )
    .await?;

    tracing::debug!(
        scope = ?ctx.scope,
        upload_id = %upload_id,
        policy_id = ctx.policy.id,
        mode = ?UploadMode::Chunked,
        chunk_size,
        total_chunks,
        folder_id = ctx.target.folder_id,
        "initialized chunked upload session"
    );

    Ok(context::chunked_upload_response(
        UploadMode::Chunked,
        upload_id,
        chunk_size,
        total_chunks,
    ))
}

async fn prepare_chunked_upload_temp_dir(state: &PrimaryAppState, upload_id: &str) -> Result<()> {
    let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, upload_id);
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::chunk_upload_failed)?;
    Ok(())
}

/// 上传协商：服务端根据存储策略决定上传模式
pub async fn init_upload(
    state: &PrimaryAppState,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    init_upload_for_scope(
        state,
        personal_scope(user_id),
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
}

/// 团队空间上传协商：规则和个人空间一致，但路径归属与配额都落在团队 scope。
pub async fn init_upload_for_team(
    state: &PrimaryAppState,
    team_id: i64,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    init_upload_for_scope(
        state,
        team_scope(team_id, user_id),
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
}
