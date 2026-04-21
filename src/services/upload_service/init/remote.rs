use chrono::{Duration, Utc};

use crate::api::constants::HOUR_SECS;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::upload_service::shared::generate_upload_id;
use crate::types::{
    DriverType, RemoteUploadStrategy, UploadMode, UploadSessionStatus, parse_storage_policy_options,
};
use crate::utils::numbers;

use super::context::{
    InitUploadContext, UploadSessionRecordParams, chunked_upload_response, direct_upload_response,
    persist_upload_session, upload_fits_single_request,
};

pub(super) async fn init_remote_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<Option<InitUploadResponse>> {
    if ctx.policy.driver_type != DriverType::Remote {
        return Ok(None);
    }

    let strategy = parse_storage_policy_options(ctx.policy.options.as_ref())
        .effective_remote_upload_strategy();
    match strategy {
        RemoteUploadStrategy::RelayStream => {
            tracing::debug!(
                scope = ?ctx.scope,
                policy_id = ctx.policy.id,
                mode = ?UploadMode::Direct,
                folder_id = ctx.target.folder_id,
                "selected remote relay stream upload mode"
            );
            Ok(Some(direct_upload_response()))
        }
        RemoteUploadStrategy::Presigned => init_presigned_remote_upload(state, ctx).await.map(Some),
    }
}

async fn init_presigned_remote_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<InitUploadResponse> {
    let driver = state.driver_registry.get_driver(&ctx.policy)?;
    let upload_id = generate_upload_id(&state.db).await?;
    let temp_key = format!("files/{upload_id}");
    let chunk_size = ctx.policy.chunk_size;

    if upload_fits_single_request(ctx.total_size, chunk_size) {
        let presigned_driver = driver.as_presigned().ok_or_else(|| {
            AsterError::storage_driver_error("presigned PUT not supported by remote driver")
        })?;
        let presigned_url = presigned_driver
            .presigned_put_url(&temp_key, std::time::Duration::from_secs(HOUR_SECS))
            .await?
            .ok_or_else(|| {
                AsterError::storage_driver_error("presigned PUT not supported by remote driver")
            })?;

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
            "initialized remote presigned upload session"
        );

        return Ok(InitUploadResponse {
            mode: UploadMode::Presigned,
            upload_id: Some(upload_id),
            chunk_size: None,
            total_chunks: None,
            presigned_url: Some(presigned_url),
        });
    }

    let multipart = state.driver_registry.get_multipart_driver(&ctx.policy)?;
    let remote_upload_id = multipart.create_multipart_upload(&temp_key).await?;
    let total_chunks = numbers::calc_total_chunks(
        ctx.total_size,
        chunk_size,
        "remote presigned multipart upload",
    )?;

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
            s3_multipart_id: Some(remote_upload_id),
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
        "initialized remote presigned multipart upload session"
    );

    Ok(chunked_upload_response(
        UploadMode::PresignedMultipart,
        upload_id,
        chunk_size,
        total_chunks,
    ))
}
