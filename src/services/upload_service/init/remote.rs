use chrono::{Duration, Utc};

use crate::api::constants::HOUR_SECS;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::upload_service::shared::{
    UPLOAD_SESSION_ID_MAX_ATTEMPTS, abort_created_multipart_upload_after_init_error,
    delete_upload_session_record_after_init_error, new_upload_id,
    upload_id_collision_exhausted_error,
};
use crate::services::workspace_storage_service::{
    PolicyUploadTransport, resolve_policy_upload_transport,
};
use crate::types::{RemoteUploadStrategy, UploadMode, UploadSessionStatus};
use crate::utils::numbers;

use super::context::{
    InitUploadContext, UploadSessionRecordParams, chunked_upload_response, direct_upload_response,
    try_persist_upload_session,
};

pub(super) async fn init_remote_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
) -> Result<Option<InitUploadResponse>> {
    let transport = resolve_policy_upload_transport(&ctx.policy);
    let PolicyUploadTransport::Remote(strategy) = transport else {
        return Ok(None);
    };
    match strategy {
        RemoteUploadStrategy::RelayStream => init_relay_stream_remote_upload(state, ctx, transport)
            .await
            .map(Some),
        RemoteUploadStrategy::Presigned => init_presigned_remote_upload(state, ctx, transport)
            .await
            .map(Some),
    }
}

async fn init_relay_stream_remote_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
    transport: PolicyUploadTransport,
) -> Result<InitUploadResponse> {
    let chunk_size = transport.effective_chunk_size(&ctx.policy);

    if transport.resolve_init_mode(&ctx.policy, ctx.total_size) == UploadMode::Direct {
        tracing::debug!(
            scope = ?ctx.scope,
            policy_id = ctx.policy.id,
            mode = ?UploadMode::Direct,
            folder_id = ctx.target.folder_id,
            "selected remote relay stream direct upload mode"
        );
        return Ok(direct_upload_response());
    }

    let multipart = state.driver_registry.get_multipart_driver(&ctx.policy)?;
    let total_chunks =
        numbers::calc_total_chunks(ctx.total_size, chunk_size, "remote relay multipart upload")?;

    init_remote_multipart_session_with_retry(
        state,
        ctx,
        multipart.as_ref(),
        MultipartSessionInitParams {
            mode: UploadMode::Chunked,
            status: UploadSessionStatus::Uploading,
            chunk_size,
            total_chunks,
            expires_in: Duration::hours(24),
            log_label: "remote relay multipart",
        },
    )
    .await
}

async fn init_presigned_remote_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
    transport: PolicyUploadTransport,
) -> Result<InitUploadResponse> {
    let driver = state.driver_registry.get_driver(&ctx.policy)?;
    let chunk_size = transport.effective_chunk_size(&ctx.policy);

    if transport.resolve_init_mode(&ctx.policy, ctx.total_size) == UploadMode::Presigned {
        return init_remote_presigned_single_upload(state, ctx, driver.as_ref()).await;
    }

    let multipart = state.driver_registry.get_multipart_driver(&ctx.policy)?;
    let total_chunks = numbers::calc_total_chunks(
        ctx.total_size,
        chunk_size,
        "remote presigned multipart upload",
    )?;

    init_remote_multipart_session_with_retry(
        state,
        ctx,
        multipart.as_ref(),
        MultipartSessionInitParams {
            mode: UploadMode::PresignedMultipart,
            status: UploadSessionStatus::Presigned,
            chunk_size,
            total_chunks,
            expires_in: Duration::hours(24),
            log_label: "remote presigned multipart",
        },
    )
    .await
}

async fn init_remote_presigned_single_upload(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
    driver: &dyn crate::storage::driver::StorageDriver,
) -> Result<InitUploadResponse> {
    for attempt in 1..=UPLOAD_SESSION_ID_MAX_ATTEMPTS {
        let upload_id = new_upload_id();
        let temp_key = format!("files/{upload_id}");
        let inserted = try_persist_upload_session(
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
                s3_temp_key: Some(temp_key.clone()),
                s3_multipart_id: None,
                expires_at: Utc::now() + Duration::hours(1),
            },
        )
        .await?;
        if !inserted {
            tracing::warn!(upload_id, attempt, "upload_id collision, retrying");
            continue;
        }

        let presigned_url = match remote_presigned_put_url(driver, &temp_key).await {
            Ok(url) => url,
            Err(error) => {
                delete_upload_session_record_after_init_error(
                    &state.db,
                    &upload_id,
                    "remote presigned URL initialization error",
                )
                .await;
                return Err(error);
            }
        };

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

    Err(upload_id_collision_exhausted_error())
}

struct MultipartSessionInitParams {
    mode: UploadMode,
    status: UploadSessionStatus,
    chunk_size: i64,
    total_chunks: i32,
    expires_in: Duration,
    log_label: &'static str,
}

async fn init_remote_multipart_session_with_retry(
    state: &PrimaryAppState,
    ctx: &InitUploadContext,
    multipart: &dyn crate::storage::multipart::MultipartStorageDriver,
    params: MultipartSessionInitParams,
) -> Result<InitUploadResponse> {
    let MultipartSessionInitParams {
        mode,
        status,
        chunk_size,
        total_chunks,
        expires_in,
        log_label,
    } = params;

    for attempt in 1..=UPLOAD_SESSION_ID_MAX_ATTEMPTS {
        let upload_id = new_upload_id();
        let temp_key = format!("files/{upload_id}");
        let remote_upload_id = multipart.create_multipart_upload(&temp_key).await?;
        let inserted_result = try_persist_upload_session(
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
                status,
                s3_temp_key: Some(temp_key.clone()),
                s3_multipart_id: Some(remote_upload_id.clone()),
                expires_at: Utc::now() + expires_in,
            },
        )
        .await;

        let inserted = match inserted_result {
            Ok(inserted) => inserted,
            Err(error) => {
                let abort_result = abort_created_multipart_upload_after_init_error(
                    multipart,
                    &temp_key,
                    &remote_upload_id,
                    &upload_id,
                    "remote upload session DB initialization error",
                )
                .await;
                if let Err(abort_error) = abort_result {
                    return Err(AsterError::storage_driver_error(format!(
                        "failed to abort remote multipart upload after DB initialization error; init error={error}, abort error={abort_error}"
                    )));
                }
                return Err(error);
            }
        };

        if !inserted {
            abort_created_multipart_upload_after_init_error(
                multipart,
                &temp_key,
                &remote_upload_id,
                &upload_id,
                "remote upload session id collision",
            )
            .await?;
            tracing::warn!(upload_id, attempt, "upload_id collision, retrying");
            continue;
        }

        tracing::debug!(
            scope = ?ctx.scope,
            upload_id = %upload_id,
            policy_id = ctx.policy.id,
            mode = ?mode,
            chunk_size,
            total_chunks,
            folder_id = ctx.target.folder_id,
            "initialized {log_label} upload session"
        );

        return Ok(chunked_upload_response(
            mode,
            upload_id,
            chunk_size,
            total_chunks,
        ));
    }

    Err(upload_id_collision_exhausted_error())
}

async fn remote_presigned_put_url(
    driver: &dyn crate::storage::driver::StorageDriver,
    temp_key: &str,
) -> Result<String> {
    let presigned_driver = driver.as_presigned().ok_or_else(|| {
        AsterError::storage_driver_error("presigned PUT not supported by remote driver")
    })?;
    presigned_driver
        .presigned_put_url(temp_key, std::time::Duration::from_secs(HOUR_SECS))
        .await?
        .ok_or_else(|| {
            AsterError::storage_driver_error("presigned PUT not supported by remote driver")
        })
}
