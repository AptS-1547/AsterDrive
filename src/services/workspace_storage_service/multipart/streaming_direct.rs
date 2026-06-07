use actix_multipart::Multipart;
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::api::api_error_code::ApiErrorCode;
use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result, file_upload_error_with_code};
use crate::runtime::{PrimaryAppState, SharedRuntimeState};
use crate::services::workspace_storage_service::{
    StorePreuploadedNondedupParams, check_quota, cleanup_preuploaded_blob_upload,
    prepare_non_dedup_blob_upload, store_preuploaded_nondedup,
};

use super::common::{
    DirectUploadParams, upload_direct_relay_shutdown_failed, upload_direct_relay_write_failed,
    upload_empty_file_error, upload_field_read_failed,
};

pub(super) async fn upload_streaming_direct(
    state: &PrimaryAppState,
    payload: &mut Multipart,
    params: DirectUploadParams<'_>,
) -> Result<file::Model> {
    let DirectUploadParams {
        scope,
        folder_id,
        relative_path,
        resolved_filename,
        policy,
        declared_size,
        actor_username,
    } = params;
    const RELAY_DIRECT_BUFFER_SIZE: usize = 64 * 1024;

    if policy.max_file_size > 0 && declared_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            declared_size, policy.max_file_size
        )));
    }

    check_quota(state.writer_db(), scope, declared_size).await?;
    let driver = state.driver_registry().get_driver(policy)?;
    let prepared_upload = prepare_non_dedup_blob_upload(policy, declared_size);
    let storage_path = prepared_upload.storage_path().to_string();

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(upload_field_read_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            let filename = if relative_path.is_some() {
                resolved_filename.to_string()
            } else {
                name
            };
            let filename = crate::utils::normalize_validate_name(&filename)?;

            let (writer, reader) = tokio::io::duplex(RELAY_DIRECT_BUFFER_SIZE);
            let upload_driver = driver.clone();
            let upload_storage_path = storage_path.clone();
            let stream_driver = upload_driver.as_stream_upload().ok_or_else(|| {
                crate::errors::AsterError::storage_driver_error("stream upload not supported")
            })?;
            let (upload_result, relay_result) = tokio::task::LocalSet::new()
                .run_until(async move {
                    let relay_task = tokio::task::spawn_local(async move {
                        let mut writer = writer;
                        while let Some(chunk) = field.next().await {
                            let chunk = chunk.map_aster_err(upload_field_read_failed)?;
                            writer.write_all(&chunk).await.map_aster_err_ctx(
                                "relay direct write",
                                upload_direct_relay_write_failed,
                            )?;
                        }
                        writer.shutdown().await.map_aster_err_ctx(
                            "relay direct shutdown",
                            upload_direct_relay_shutdown_failed,
                        )?;
                        Ok::<(), AsterError>(())
                    });

                    let upload_result = stream_driver
                        .put_reader(&upload_storage_path, Box::new(reader), declared_size)
                        .await;
                    let relay_result = relay_task.await.map_err(|err| {
                        file_upload_error_with_code(
                            ApiErrorCode::UploadDirectRelayTaskFailed,
                            format!("relay direct task failed: {err}"),
                        )
                    })?;

                    Ok::<(Result<String>, Result<()>), AsterError>((upload_result, relay_result))
                })
                .await?;

            if let Err(err) = upload_result {
                cleanup_preuploaded_blob_upload(
                    driver.as_ref(),
                    &prepared_upload,
                    "direct stream upload error",
                )
                .await;
                return Err(err);
            }

            if let Err(err) = relay_result {
                cleanup_preuploaded_blob_upload(
                    driver.as_ref(),
                    &prepared_upload,
                    "direct stream relay error",
                )
                .await;
                return Err(err);
            }

            return store_preuploaded_nondedup(
                state,
                StorePreuploadedNondedupParams {
                    scope,
                    folder_id,
                    filename: &filename,
                    size: declared_size,
                    existing_file_id: None,
                    skip_lock_check: false,
                    policy,
                    preuploaded_blob: prepared_upload,
                    actor_username,
                },
            )
            .await;
        }
    }

    Err(upload_empty_file_error())
}
