use chrono::{Duration, Utc};

use crate::api::constants::HOUR_SECS;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::upload_service::shared::generate_upload_id;
use crate::types::{
    DriverType, S3UploadStrategy, UploadMode, UploadSessionStatus,
    effective_s3_multipart_chunk_size, parse_storage_policy_options,
};
use crate::utils::numbers;

use super::context::{
    InitUploadContext, UploadSessionRecordParams, chunked_upload_response, direct_upload_response,
    persist_upload_session, upload_fits_single_request,
};

pub(super) async fn init_s3_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<Option<InitUploadResponse>> {
    if ctx.policy.driver_type != DriverType::S3 {
        return Ok(None);
    }

    let strategy =
        parse_storage_policy_options(ctx.policy.options.as_ref()).effective_s3_upload_strategy();
    match strategy {
        S3UploadStrategy::Presigned => init_presigned_s3_upload(state, ctx).await.map(Some),
        S3UploadStrategy::RelayStream => init_relay_stream_s3_upload(state, ctx).await.map(Some),
    }
}

async fn init_presigned_s3_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<InitUploadResponse> {
    let driver = state.driver_registry.get_driver(&ctx.policy)?;
    let upload_id = generate_upload_id(&state.db).await?;
    let temp_key = format!("files/{upload_id}");
    let chunk_size = effective_s3_multipart_chunk_size(ctx.policy.chunk_size);

    // 小文件 presigned：客户端直接 PUT 到最终 temp object，不经过服务端 relay，
    // 也不需要 chunk bookkeeping。
    if upload_fits_single_request(ctx.total_size, chunk_size) {
        let presigned_url = presigned_put_url(driver.as_ref(), &temp_key).await?;
        persist_upload_session(
            &state.db,
            UploadSessionRecordParams {
                upload_id: upload_id.clone(),
                scope: ctx.scope,
                filename: ctx.target.filename.clone(),
                total_size: ctx.total_size,
                chunk_size: 0,
                total_chunks: 0,
                folder_id: ctx.target.folder_id,
                policy_id: ctx.policy.id,
                status: UploadSessionStatus::Presigned,
                s3_temp_key: Some(temp_key),
                s3_multipart_id: None,
                expires_at: Utc::now() + Duration::hours(1),
            },
        )
        .await?;

        tracing::debug!(
            scope = ?ctx.scope,
            upload_id = %upload_id,
            policy_id = ctx.policy.id,
            mode = ?UploadMode::Presigned,
            folder_id = ctx.target.folder_id,
            "initialized presigned upload session"
        );

        return Ok(InitUploadResponse {
            mode: UploadMode::Presigned,
            upload_id: Some(upload_id),
            chunk_size: None,
            total_chunks: None,
            presigned_url: Some(presigned_url),
        });
    }

    // 大文件 presigned multipart：服务端仍然不接管数据流，但必须保留 session，
    // 用来记录 multipart upload_id、分片总数以及后续 complete 阶段的收口点。
    let multipart = state.driver_registry.get_multipart_driver(&ctx.policy)?;
    let s3_upload_id = multipart.create_multipart_upload(&temp_key).await?;
    let total_chunks =
        numbers::calc_total_chunks(ctx.total_size, chunk_size, "presigned multipart upload")?;

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
            status: UploadSessionStatus::Presigned,
            s3_temp_key: Some(temp_key),
            s3_multipart_id: Some(s3_upload_id),
            expires_at: Utc::now() + Duration::hours(24),
        },
    )
    .await?;

    tracing::debug!(
        scope = ?ctx.scope,
        upload_id = %upload_id,
        policy_id = ctx.policy.id,
        mode = ?UploadMode::PresignedMultipart,
        chunk_size,
        total_chunks,
        folder_id = ctx.target.folder_id,
        "initialized presigned multipart upload session"
    );

    Ok(chunked_upload_response(
        UploadMode::PresignedMultipart,
        upload_id,
        chunk_size,
        total_chunks,
    ))
}

async fn init_relay_stream_s3_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<InitUploadResponse> {
    let chunk_size = effective_s3_multipart_chunk_size(ctx.policy.chunk_size);

    // relay_stream + 小文件：直接走普通上传接口，让服务端把字节流转发到驱动。
    if upload_fits_single_request(ctx.total_size, chunk_size) {
        tracing::debug!(
            scope = ?ctx.scope,
            policy_id = ctx.policy.id,
            mode = ?UploadMode::Direct,
            folder_id = ctx.target.folder_id,
            "selected direct relay upload mode"
        );
        return Ok(direct_upload_response());
    }

    // relay_stream + 大文件：客户端仍然分片传给服务端，服务端再逐片上传到 S3 multipart。
    let multipart = state.driver_registry.get_multipart_driver(&ctx.policy)?;
    let upload_id = generate_upload_id(&state.db).await?;
    let temp_key = format!("files/{upload_id}");
    let s3_upload_id = multipart.create_multipart_upload(&temp_key).await?;
    let total_chunks =
        numbers::calc_total_chunks(ctx.total_size, chunk_size, "relay multipart upload")?;

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
            s3_temp_key: Some(temp_key),
            s3_multipart_id: Some(s3_upload_id),
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
        "initialized relay multipart upload session"
    );

    Ok(chunked_upload_response(
        UploadMode::Chunked,
        upload_id,
        chunk_size,
        total_chunks,
    ))
}

async fn presigned_put_url(
    driver: &dyn crate::storage::driver::StorageDriver,
    temp_key: &str,
) -> Result<String> {
    let presigned_driver = driver
        .as_presigned()
        .ok_or_else(|| AsterError::storage_driver_error("presigned PUT not supported by driver"))?;
    presigned_driver
        .presigned_put_url(temp_key, std::time::Duration::from_secs(HOUR_SECS))
        .await?
        .ok_or_else(|| AsterError::storage_driver_error("presigned PUT not supported by driver"))
}
