//! 工作空间存储服务子模块：`multipart`。

use actix_multipart::Multipart;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::entities::file;
use crate::errors::{
    AsterError, MapAsterErr, Result, file_upload_error_with_subcode, validation_error_with_subcode,
};
use crate::runtime::PrimaryAppState;
use crate::types::DriverType;

use super::{
    StoreFromTempHints, StoreFromTempParams, StorePreuploadedNondedupParams, WorkspaceStorageScope,
    check_quota, cleanup_preuploaded_blob_upload, create_empty, ensure_upload_parent_path,
    local_content_dedup_enabled, parse_relative_upload_path, prepare_non_dedup_blob_upload,
    resolve_policy_for_size_with_verified_folder, store_from_temp_with_hints,
    store_preuploaded_nondedup, streaming_direct_upload_eligible, verify_folder_access,
};
use crate::utils::numbers::usize_to_i64;

#[derive(Clone, Copy)]
struct DirectUploadParams<'a> {
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    relative_path: Option<&'a str>,
    resolved_filename: &'a str,
    policy: &'a crate::entities::storage_policy::Model,
    declared_size: i64,
    actor_username: Option<&'a str>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct WorkspaceUploadHints<'a> {
    pub actor_username: Option<&'a str>,
}

fn upload_field_read_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.field_read_failed", message)
}

fn upload_local_staging_path_resolve_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_path_resolve_failed", message)
}

fn upload_local_staging_dir_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_dir_create_failed", message)
}

fn upload_local_staging_file_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_file_create_failed", message)
}

fn upload_local_staging_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_write_failed", message)
}

fn upload_local_staging_flush_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_flush_failed", message)
}

fn upload_direct_relay_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.direct_relay_write_failed", message)
}

fn upload_direct_relay_shutdown_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.direct_relay_shutdown_failed", message)
}

fn upload_temp_dir_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_dir_create_failed", message)
}

fn upload_temp_file_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_create_failed", message)
}

fn upload_temp_file_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_write_failed", message)
}

fn upload_temp_file_flush_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_flush_failed", message)
}

fn upload_empty_file_error() -> AsterError {
    validation_error_with_subcode("upload.empty_file", "empty file")
}

fn upload_size_mismatch_error(declared_size: i64, actual_size: i64) -> AsterError {
    AsterError::validation_error(format!(
        "size mismatch: declared {} bytes, received {} bytes",
        declared_size, actual_size
    ))
}

async fn upload_local_direct(
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
    let should_dedup = local_content_dedup_enabled(policy);

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

            let staging_token = format!("{}.upload", crate::utils::id::new_uuid());
            let staging_path =
                crate::storage::drivers::local::upload_staging_path(policy, &staging_token)
                    .map_aster_err_ctx(
                        "resolve local staging path",
                        upload_local_staging_path_resolve_failed,
                    )?;
            if let Some(parent) = staging_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_aster_err_ctx(
                    "create local staging dir",
                    upload_local_staging_dir_create_failed,
                )?;
            }

            let staging_file = tokio::fs::File::create(&staging_path)
                .await
                .map_aster_err_ctx(
                    "create local staging file",
                    upload_local_staging_file_create_failed,
                )?;
            let mut staging_file = BufWriter::new(staging_file);
            let mut hasher = should_dedup.then(Sha256::new);
            let mut size: i64 = 0;
            let staging_path = staging_path.to_string_lossy().into_owned();

            let write_result = async {
                while let Some(chunk) = field.next().await {
                    let chunk = chunk.map_aster_err(upload_field_read_failed)?;
                    if let Some(hasher) = hasher.as_mut() {
                        hasher.update(&chunk);
                    }
                    staging_file.write_all(&chunk).await.map_aster_err_ctx(
                        "write local staging file",
                        upload_local_staging_write_failed,
                    )?;
                    size = size
                        .checked_add(usize_to_i64(chunk.len(), "chunk length")?)
                        .ok_or_else(|| {
                            file_upload_error_with_subcode(
                                "upload.body_size_overflow",
                                "accumulated chunk size overflows i64",
                            )
                        })?;
                }
                staging_file.flush().await.map_aster_err_ctx(
                    "flush local staging file",
                    upload_local_staging_flush_failed,
                )?;
                Ok::<(), AsterError>(())
            }
            .await;

            drop(staging_file);

            if let Err(err) = write_result {
                crate::utils::cleanup_temp_file(&staging_path).await;
                return Err(err);
            }

            if size != declared_size {
                crate::utils::cleanup_temp_file(&staging_path).await;
                return Err(upload_size_mismatch_error(declared_size, size));
            }

            if size == 0 {
                crate::utils::cleanup_temp_file(&staging_path).await;
                return create_empty(state, scope, folder_id, &filename).await;
            }

            let precomputed_hash =
                hasher.map(|hasher| crate::utils::hash::sha256_digest_to_hex(&hasher.finalize()));
            let resolved_policy = Some(policy.clone());
            let result = store_from_temp_with_hints(
                state,
                StoreFromTempParams::new(scope, folder_id, &filename, &staging_path, size),
                StoreFromTempHints {
                    resolved_policy,
                    precomputed_hash: precomputed_hash.as_deref(),
                    actor_username,
                },
            )
            .await;

            crate::utils::cleanup_temp_file(&staging_path).await;
            return result;
        }
    }

    Err(upload_empty_file_error())
}

async fn upload_streaming_direct(
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

    check_quota(&state.db, scope, declared_size).await?;
    let driver = state.driver_registry.get_driver(policy)?;
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
                        file_upload_error_with_subcode(
                            "upload.direct_relay_task_failed",
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

            return match store_preuploaded_nondedup(
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
            .await
            {
                Ok(file) => Ok(file),
                Err(err) => Err(err),
            };
        }
    }

    Err(upload_empty_file_error())
}

pub(crate) async fn upload(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
) -> Result<file::Model> {
    upload_with_hints(
        state,
        scope,
        payload,
        folder_id,
        relative_path,
        declared_size,
        WorkspaceUploadHints::default(),
    )
    .await
}

pub(crate) async fn upload_with_hints(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
    hints: WorkspaceUploadHints<'_>,
) -> Result<file::Model> {
    tracing::debug!(
        scope = ?scope,
        folder_id,
        relative_path = relative_path.unwrap_or(""),
        declared_size,
        "starting multipart upload"
    );

    if let Some(declared_size) = declared_size
        && declared_size < 0
    {
        return Err(validation_error_with_subcode(
            "upload.declared_size_invalid",
            "declared_size cannot be negative",
        ));
    }

    let (effective_folder_id, effective_folder, resolved_filename) = match relative_path {
        Some(path) => {
            let parsed = parse_relative_upload_path(state, scope, folder_id, path).await?;
            let resolved_parent =
                ensure_upload_parent_path(state, scope, &parsed, hints.actor_username).await?;
            (
                resolved_parent.folder_id,
                resolved_parent.folder,
                parsed.filename,
            )
        }
        None => {
            let folder = match folder_id {
                Some(folder_id) => {
                    Some(verify_folder_access(state, scope, folder_id).await?.into())
                }
                None => None,
            };
            (folder_id, folder, String::new())
        }
    };

    tracing::debug!(
        scope = ?scope,
        folder_id = effective_folder_id,
        resolved_filename = %resolved_filename,
        has_relative_path = relative_path.is_some(),
        "resolved upload target"
    );

    if let Some(declared_size) = declared_size {
        let policy = resolve_policy_for_size_with_verified_folder(
            state,
            scope,
            effective_folder,
            declared_size,
        )
        .await?;
        if streaming_direct_upload_eligible(&policy, declared_size) {
            tracing::debug!(
                scope = ?scope,
                folder_id = effective_folder_id,
                resolved_filename = %resolved_filename,
                policy_id = policy.id,
                driver_type = ?policy.driver_type,
                declared_size,
                "using streaming direct upload fast path"
            );

            let result = upload_streaming_direct(
                state,
                payload,
                DirectUploadParams {
                    scope,
                    folder_id: effective_folder_id,
                    relative_path,
                    resolved_filename: &resolved_filename,
                    policy: &policy,
                    declared_size,
                    actor_username: hints.actor_username,
                },
            )
            .await;
            if let Ok(file) = &result {
                tracing::debug!(
                    scope = ?scope,
                    file_id = file.id,
                    folder_id = file.folder_id,
                    size = file.size,
                    "completed streaming direct upload"
                );
            }
            return result;
        }
        if policy.driver_type == DriverType::Local {
            tracing::debug!(
                scope = ?scope,
                folder_id = effective_folder_id,
                resolved_filename = %resolved_filename,
                policy_id = policy.id,
                driver_type = ?policy.driver_type,
                declared_size,
                "using local direct upload fast path"
            );

            let result = upload_local_direct(
                state,
                payload,
                DirectUploadParams {
                    scope,
                    folder_id: effective_folder_id,
                    relative_path,
                    resolved_filename: &resolved_filename,
                    policy: &policy,
                    declared_size,
                    actor_username: hints.actor_username,
                },
            )
            .await;
            if let Ok(file) = &result {
                tracing::debug!(
                    scope = ?scope,
                    file_id = file.id,
                    folder_id = file.folder_id,
                    size = file.size,
                    "completed local direct upload"
                );
            }
            return result;
        }
    }

    let mut filename = String::from("unnamed");
    let mut saw_file_field = false;
    let temp_dir = &state.config.server.temp_dir;
    let runtime_temp_dir = crate::utils::paths::runtime_temp_dir(temp_dir);
    let temp_path =
        crate::utils::paths::runtime_temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&runtime_temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", upload_temp_dir_create_failed)?;

    let temp_file = tokio::fs::File::create(&temp_path)
        .await
        .map_aster_err_ctx("create temp", upload_temp_file_create_failed)?;
    let mut temp_file = BufWriter::new(temp_file);
    let mut size: i64 = 0;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(upload_field_read_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            saw_file_field = true;
            filename = if relative_path.is_some() {
                resolved_filename.clone()
            } else {
                name
            };

            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_aster_err(upload_field_read_failed)?;
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_aster_err_ctx("write temp", upload_temp_file_write_failed)?;
                size = size
                    .checked_add(usize_to_i64(chunk.len(), "chunk length")?)
                    .ok_or_else(|| {
                        file_upload_error_with_subcode(
                            "upload.body_size_overflow",
                            "accumulated chunk size overflows i64",
                        )
                    })?;
            }
            break;
        }
    }

    temp_file
        .flush()
        .await
        .map_aster_err_ctx("flush temp", upload_temp_file_flush_failed)?;
    drop(temp_file);

    if !saw_file_field {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return Err(upload_empty_file_error());
    }

    if let Some(declared_size) = declared_size
        && size != declared_size
    {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return Err(upload_size_mismatch_error(declared_size, size));
    }

    if size == 0 {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return create_empty(state, scope, effective_folder_id, &filename).await;
    }

    let result = store_from_temp_with_hints(
        state,
        StoreFromTempParams::new(scope, effective_folder_id, &filename, &temp_path, size),
        StoreFromTempHints {
            resolved_policy: Some(
                resolve_policy_for_size_with_verified_folder(state, scope, effective_folder, size)
                    .await?,
            ),
            actor_username: hints.actor_username,
            ..Default::default()
        },
    )
    .await;

    crate::utils::cleanup_temp_file(&temp_path).await;
    if let Ok(file) = &result {
        tracing::debug!(
            scope = ?scope,
            file_id = file.id,
            folder_id = file.folder_id,
            size = file.size,
            "completed staged multipart upload"
        );
    }
    result
}
