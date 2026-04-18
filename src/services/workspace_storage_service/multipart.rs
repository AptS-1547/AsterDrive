//! 工作空间存储服务子模块：`multipart`。

use actix_multipart::Multipart;
use chrono::Utc;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::storage_change_service;
use crate::types::{
    DriverType, S3UploadStrategy, effective_s3_multipart_chunk_size, parse_storage_policy_options,
};

use super::{
    StoreFromTempHints, StoreFromTempParams, WorkspaceStorageScope, check_quota,
    create_new_file_from_blob, create_s3_nondedup_blob, ensure_upload_parent_path,
    local_content_dedup_enabled, parse_relative_upload_path, resolve_policy_for_size,
    store_from_temp, store_from_temp_with_hints, update_storage_used, verify_folder_access,
};
use crate::utils::numbers::usize_to_i64;

pub(crate) fn relay_stream_direct_upload_eligible(
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> bool {
    if declared_size <= 0 || policy.driver_type != DriverType::S3 {
        return false;
    }

    let options = parse_storage_policy_options(policy.options.as_ref());
    if options.effective_s3_upload_strategy() != S3UploadStrategy::RelayStream {
        return false;
    }

    policy.chunk_size == 0 || declared_size <= effective_s3_multipart_chunk_size(policy.chunk_size)
}

#[derive(Clone, Copy)]
struct DirectUploadParams<'a> {
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    relative_path: Option<&'a str>,
    resolved_filename: &'a str,
    policy: &'a crate::entities::storage_policy::Model,
    declared_size: i64,
}

async fn upload_local_direct(
    state: &AppState,
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
    } = params;
    let should_dedup = local_content_dedup_enabled(policy);

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            let filename = if relative_path.is_some() {
                resolved_filename.to_string()
            } else {
                name
            };
            crate::utils::validate_name(&filename)?;

            let staging_token = format!("{}.upload", crate::utils::id::new_uuid());
            let staging_path =
                crate::storage::drivers::local::upload_staging_path(policy, &staging_token)
                    .map_aster_err_ctx(
                        "resolve local staging path",
                        AsterError::file_upload_failed,
                    )?;
            if let Some(parent) = staging_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_aster_err_ctx(
                    "create local staging dir",
                    AsterError::file_upload_failed,
                )?;
            }

            let mut staging_file = tokio::fs::File::create(&staging_path)
                .await
                .map_aster_err_ctx("create local staging file", AsterError::file_upload_failed)?;
            let mut hasher = should_dedup.then(Sha256::new);
            let mut size: i64 = 0;
            let staging_path = staging_path.to_string_lossy().into_owned();

            let write_result = async {
                while let Some(chunk) = field.next().await {
                    let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                    if let Some(hasher) = hasher.as_mut() {
                        hasher.update(&chunk);
                    }
                    staging_file.write_all(&chunk).await.map_aster_err_ctx(
                        "write local staging file",
                        AsterError::file_upload_failed,
                    )?;
                    size = size
                        .checked_add(usize_to_i64(chunk.len(), "chunk length")?)
                        .ok_or_else(|| {
                            AsterError::file_upload_failed("accumulated chunk size overflows i64")
                        })?;
                }
                staging_file.flush().await.map_aster_err_ctx(
                    "flush local staging file",
                    AsterError::file_upload_failed,
                )?;
                Ok::<(), AsterError>(())
            }
            .await;

            drop(staging_file);

            if let Err(err) = write_result {
                crate::utils::cleanup_temp_file(&staging_path).await;
                return Err(err);
            }

            if size == 0 {
                crate::utils::cleanup_temp_file(&staging_path).await;
                return Err(AsterError::validation_error("empty file"));
            }

            let precomputed_hash =
                hasher.map(|hasher| crate::utils::hash::sha256_digest_to_hex(&hasher.finalize()));
            let resolved_policy = (size == declared_size).then_some(policy.clone());
            let result = store_from_temp_with_hints(
                state,
                StoreFromTempParams::new(scope, folder_id, &filename, &staging_path, size),
                StoreFromTempHints {
                    resolved_policy,
                    precomputed_hash: precomputed_hash.as_deref(),
                },
            )
            .await;

            crate::utils::cleanup_temp_file(&staging_path).await;
            return result;
        }
    }

    Err(AsterError::validation_error("empty file"))
}

async fn upload_s3_relay_direct(
    state: &AppState,
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

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            let filename = if relative_path.is_some() {
                resolved_filename.to_string()
            } else {
                name
            };
            crate::utils::validate_name(&filename)?;

            let upload_id = crate::utils::id::new_uuid();
            let storage_path = format!("files/{upload_id}");
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
                            let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                            writer.write_all(&chunk).await.map_aster_err_ctx(
                                "relay direct write",
                                AsterError::file_upload_failed,
                            )?;
                        }
                        writer.shutdown().await.map_aster_err_ctx(
                            "relay direct shutdown",
                            AsterError::file_upload_failed,
                        )?;
                        Ok::<(), AsterError>(())
                    });

                    let upload_result = stream_driver
                        .put_reader(&upload_storage_path, Box::new(reader), declared_size)
                        .await;
                    let relay_result = relay_task.await.map_err(|err| {
                        AsterError::file_upload_failed(format!("relay direct task failed: {err}"))
                    })?;

                    Ok::<(Result<String>, Result<()>), AsterError>((upload_result, relay_result))
                })
                .await?;

            if let Err(err) = upload_result {
                if let Err(cleanup_err) = driver.delete(&storage_path).await {
                    tracing::warn!(
                        "failed to cleanup relay direct object {} after upload error: {cleanup_err}",
                        storage_path
                    );
                }
                return Err(err);
            }

            if let Err(err) = relay_result {
                if let Err(cleanup_err) = driver.delete(&storage_path).await {
                    tracing::warn!(
                        "failed to cleanup relay direct object {} after relay error: {cleanup_err}",
                        storage_path
                    );
                }
                return Err(err);
            }

            let now = Utc::now();
            let txn = crate::db::transaction::begin(&state.db).await?;
            let create_result = async {
                check_quota(&txn, scope, declared_size).await?;
                let blob =
                    create_s3_nondedup_blob(&txn, declared_size, policy.id, &upload_id).await?;
                let created =
                    create_new_file_from_blob(&txn, scope, folder_id, &filename, &blob, now)
                        .await?;
                update_storage_used(&txn, scope, declared_size).await?;
                crate::db::transaction::commit(txn).await?;
                Ok::<file::Model, AsterError>(created)
            }
            .await;

            return match create_result {
                Ok(file) => {
                    storage_change_service::publish(
                        state,
                        storage_change_service::StorageChangeEvent::new(
                            storage_change_service::StorageChangeKind::FileCreated,
                            scope,
                            vec![file.id],
                            vec![],
                            vec![file.folder_id],
                        ),
                    );
                    Ok(file)
                }
                Err(err) => {
                    if let Err(cleanup_err) = driver.delete(&storage_path).await {
                        tracing::warn!(
                            "failed to cleanup relay direct object {} after DB error: {cleanup_err}",
                            storage_path
                        );
                    }
                    Err(err)
                }
            };
        }
    }

    Err(AsterError::validation_error("empty file"))
}

pub(crate) async fn upload(
    state: &AppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
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
        return Err(AsterError::validation_error(
            "declared_size cannot be negative",
        ));
    }

    let (resolved_folder_id, resolved_filename) = match relative_path {
        Some(path) => {
            let parsed = parse_relative_upload_path(state, scope, folder_id, path).await?;
            let resolved_folder_id = ensure_upload_parent_path(state, scope, &parsed).await?;
            (resolved_folder_id, parsed.filename)
        }
        None => {
            if let Some(folder_id) = folder_id {
                verify_folder_access(state, scope, folder_id).await?;
            }
            (folder_id, String::new())
        }
    };

    let effective_folder_id = if relative_path.is_some() {
        resolved_folder_id
    } else {
        folder_id
    };

    tracing::debug!(
        scope = ?scope,
        folder_id = effective_folder_id,
        resolved_filename = %resolved_filename,
        has_relative_path = relative_path.is_some(),
        "resolved upload target"
    );

    if let Some(declared_size) = declared_size {
        let policy =
            resolve_policy_for_size(state, scope, effective_folder_id, declared_size).await?;
        if relay_stream_direct_upload_eligible(&policy, declared_size) {
            tracing::debug!(
                scope = ?scope,
                folder_id = effective_folder_id,
                resolved_filename = %resolved_filename,
                policy_id = policy.id,
                driver_type = ?policy.driver_type,
                declared_size,
                "using relay direct upload fast path"
            );

            let result = upload_s3_relay_direct(
                state,
                payload,
                DirectUploadParams {
                    scope,
                    folder_id: effective_folder_id,
                    relative_path,
                    resolved_filename: &resolved_filename,
                    policy: &policy,
                    declared_size,
                },
            )
            .await;
            if let Ok(file) = &result {
                tracing::debug!(
                    scope = ?scope,
                    file_id = file.id,
                    folder_id = file.folder_id,
                    size = file.size,
                    "completed relay direct upload"
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
    let temp_dir = &state.config.server.temp_dir;
    let runtime_temp_dir = crate::utils::paths::runtime_temp_dir(temp_dir);
    let temp_path =
        crate::utils::paths::runtime_temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&runtime_temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::file_upload_failed)?;

    let mut temp_file = tokio::fs::File::create(&temp_path)
        .await
        .map_aster_err_ctx("create temp", AsterError::file_upload_failed)?;
    let mut size: i64 = 0;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|content| content.get_filename().map(|name| name.to_string()));

        if let Some(name) = is_file {
            filename = if relative_path.is_some() {
                resolved_filename.clone()
            } else {
                name
            };

            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_aster_err_ctx("write temp", AsterError::file_upload_failed)?;
                size = size
                    .checked_add(usize_to_i64(chunk.len(), "chunk length")?)
                    .ok_or_else(|| {
                        AsterError::file_upload_failed("accumulated chunk size overflows i64")
                    })?;
            }
            break;
        }
    }

    temp_file
        .flush()
        .await
        .map_aster_err_ctx("flush temp", AsterError::file_upload_failed)?;
    drop(temp_file);

    if size == 0 {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return Err(AsterError::validation_error("empty file"));
    }

    let result = store_from_temp(
        state,
        StoreFromTempParams::new(scope, effective_folder_id, &filename, &temp_path, size),
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
