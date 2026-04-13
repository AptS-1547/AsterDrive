use actix_multipart::Multipart;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use tokio::io::AsyncWriteExt;

use crate::db::repository::file_repo;
use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::storage_change_service;
use crate::types::{
    DriverType, S3UploadStrategy, effective_s3_multipart_chunk_size, parse_storage_policy_options,
};
use sha2::{Digest, Sha256};

pub(crate) use crate::services::workspace_scope_service::{
    WorkspaceStorageScope, ensure_active_file_scope, ensure_active_folder_scope, ensure_file_scope,
    ensure_folder_scope, ensure_personal_file_scope, list_files_in_folder, list_folders_in_parent,
    require_scope_access, require_team_access, require_team_management_access, verify_file_access,
    verify_folder_access,
};
pub(crate) use crate::services::workspace_storage_core::{
    check_quota, create_exact_file_from_blob, create_new_file_from_blob, create_nondedup_blob,
    create_s3_nondedup_blob, ensure_upload_parent_path, finalize_upload_session_blob,
    finalize_upload_session_file, load_storage_limits, local_content_dedup_enabled,
    parse_relative_upload_path, resolve_policy_for_size, update_storage_used,
};

const HASH_BUF_SIZE: usize = 65536;

#[derive(Clone, Copy)]
enum NewFileMode {
    ResolveUnique,
    Exact,
}

fn relay_stream_direct_upload_eligible(
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> bool {
    if declared_size <= 0 || policy.driver_type != DriverType::S3 {
        return false;
    }

    let options = parse_storage_policy_options(&policy.options);
    if options.effective_s3_upload_strategy() != S3UploadStrategy::RelayStream {
        return false;
    }

    policy.chunk_size == 0 || declared_size <= effective_s3_multipart_chunk_size(policy.chunk_size)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn store_from_temp(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
) -> Result<file::Model> {
    store_from_temp_internal(
        state,
        scope,
        folder_id,
        filename,
        temp_path,
        size,
        existing_file_id,
        skip_lock_check,
        None,
        None,
        NewFileMode::ResolveUnique,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn store_from_temp_with_hints(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
    resolved_policy: Option<crate::entities::storage_policy::Model>,
    precomputed_hash: Option<&str>,
) -> Result<file::Model> {
    store_from_temp_internal(
        state,
        scope,
        folder_id,
        filename,
        temp_path,
        size,
        existing_file_id,
        skip_lock_check,
        resolved_policy,
        precomputed_hash,
        NewFileMode::ResolveUnique,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn store_from_temp_exact_name_with_hints(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
    resolved_policy: Option<crate::entities::storage_policy::Model>,
    precomputed_hash: Option<&str>,
) -> Result<file::Model> {
    store_from_temp_internal(
        state,
        scope,
        folder_id,
        filename,
        temp_path,
        size,
        existing_file_id,
        skip_lock_check,
        resolved_policy,
        precomputed_hash,
        NewFileMode::Exact,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn store_from_temp_internal(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
    resolved_policy: Option<crate::entities::storage_policy::Model>,
    precomputed_hash: Option<&str>,
    new_file_mode: NewFileMode,
) -> Result<file::Model> {
    let db = &state.db;

    tracing::debug!(
        scope = ?scope,
        folder_id,
        filename = %filename,
        size,
        existing_file_id,
        skip_lock_check,
        policy_hint = resolved_policy.as_ref().map(|policy| policy.id),
        has_precomputed_hash = precomputed_hash.is_some(),
        "storing file from temp"
    );

    crate::utils::validate_name(filename)?;

    let policy = match resolved_policy {
        Some(policy) => policy,
        None => resolve_policy_for_size(state, scope, folder_id, size).await?,
    };
    let should_dedup = local_content_dedup_enabled(&policy);

    tracing::debug!(
        scope = ?scope,
        policy_id = policy.id,
        driver_type = ?policy.driver_type,
        should_dedup,
        "resolved storage policy for temp file"
    );

    if policy.max_file_size > 0 && size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            size, policy.max_file_size
        )));
    }

    let now = Utc::now();
    let driver = state.driver_registry.get_driver(&policy)?;

    let dedup_target = if should_dedup {
        use tokio::io::AsyncReadExt;

        let file_hash = match precomputed_hash {
            Some(file_hash) => file_hash.to_string(),
            None => {
                let mut hasher = Sha256::new();
                let mut reader = tokio::fs::File::open(temp_path)
                    .await
                    .map_aster_err_ctx("open temp", AsterError::file_upload_failed)?;
                let mut buf = vec![0u8; HASH_BUF_SIZE];
                loop {
                    let n = reader
                        .read(&mut buf)
                        .await
                        .map_aster_err_ctx("read temp", AsterError::file_upload_failed)?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buf[..n]);
                }
                crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
            }
        };
        let storage_path = crate::utils::storage_path_from_hash(&file_hash);
        Some((file_hash, storage_path))
    } else {
        None
    };

    let overwrite_ctx = if let Some(existing_id) = existing_file_id {
        let old_file = verify_file_access(state, scope, existing_id).await?;
        if old_file.is_locked && !skip_lock_check {
            return Err(AsterError::resource_locked("file is locked"));
        }
        let old_blob = file_repo::find_blob_by_id(db, old_file.blob_id).await?;
        if let Err(err) =
            crate::services::thumbnail_service::delete_thumbnail(state, &old_blob).await
        {
            tracing::warn!("failed to delete thumbnail for blob {}: {err}", old_blob.id);
        }
        Some((old_file, old_blob))
    } else {
        None
    };
    // 覆盖写入会把旧内容转成历史版本保留，因此逻辑占用始终新增整份新内容，
    // 不能只按 current file 的 size 差值计费。
    let storage_delta = overwrite_ctx.as_ref().map_or(size, |_| size);

    if storage_delta > 0 {
        check_quota(db, scope, storage_delta).await?;
    }

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    if storage_delta > 0 {
        check_quota(&txn, scope, storage_delta).await?;
    }

    let blob = if let Some((file_hash, storage_path)) = dedup_target.as_ref() {
        let blob =
            file_repo::find_or_create_blob(&txn, file_hash, size, policy.id, storage_path).await?;
        if blob.inserted {
            driver.put_file(storage_path, temp_path).await?;
        }
        blob.model
    } else if policy.driver_type == crate::types::DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        let blob = create_s3_nondedup_blob(&txn, size, policy.id, &upload_id).await?;
        driver.put_file(&blob.storage_path, temp_path).await?;
        blob
    } else {
        let blob = create_nondedup_blob(&txn, size, policy.id).await?;
        driver.put_file(&blob.storage_path, temp_path).await?;
        blob
    };

    let result = if let Some((old_file, old_blob)) = overwrite_ctx {
        let existing_id = old_file.id;
        let mut active: file::ActiveModel = old_file.into();
        active.blob_id = Set(blob.id);
        active.size = Set(blob.size);
        active.mime_type = Set(mime);
        active.updated_at = Set(now);
        let updated = active.update(&txn).await.map_err(AsterError::from)?;

        let next_ver = crate::db::repository::version_repo::next_version(&txn, existing_id).await?;
        crate::db::repository::version_repo::create(
            &txn,
            crate::entities::file_version::ActiveModel {
                file_id: Set(existing_id),
                blob_id: Set(old_blob.id),
                version: Set(next_ver),
                size: Set(old_blob.size),
                created_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        if storage_delta != 0 {
            update_storage_used(&txn, scope, storage_delta).await?;
        }
        updated
    } else {
        let created = match new_file_mode {
            NewFileMode::ResolveUnique => {
                create_new_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?
            }
            NewFileMode::Exact => {
                create_exact_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?
            }
        };
        if storage_delta != 0 {
            update_storage_used(&txn, scope, storage_delta).await?;
        }
        created
    };

    txn.commit().await.map_err(AsterError::from)?;

    let event_kind = if existing_file_id.is_some() {
        storage_change_service::StorageChangeKind::FileUpdated
    } else {
        storage_change_service::StorageChangeKind::FileCreated
    };
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            event_kind,
            scope,
            vec![result.id],
            vec![],
            vec![result.folder_id],
        ),
    );

    if let Some(existing_id) = existing_file_id {
        crate::services::version_service::cleanup_excess(state, existing_id).await?;
    }

    tracing::debug!(
        scope = ?scope,
        file_id = result.id,
        blob_id = result.blob_id,
        folder_id = result.folder_id,
        overwritten = existing_file_id.is_some(),
        size = result.size,
        "stored file from temp"
    );

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
async fn upload_local_direct(
    state: &AppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    resolved_filename: &str,
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> Result<file::Model> {
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
            let staging_path = crate::storage::local::upload_staging_path(policy, &staging_token);
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
                    size += chunk.len() as i64;
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
                scope,
                folder_id,
                &filename,
                &staging_path,
                size,
                None,
                false,
                resolved_policy,
                precomputed_hash.as_deref(),
            )
            .await;

            crate::utils::cleanup_temp_file(&staging_path).await;
            return result;
        }
    }

    Err(AsterError::validation_error("empty file"))
}

#[allow(clippy::too_many_arguments)]
async fn upload_s3_relay_direct(
    state: &AppState,
    scope: WorkspaceStorageScope,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    resolved_filename: &str,
    policy: &crate::entities::storage_policy::Model,
    declared_size: i64,
) -> Result<file::Model> {
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

                    let upload_result = upload_driver
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
            let txn = state.db.begin().await.map_err(AsterError::from)?;
            let create_result = async {
                check_quota(&txn, scope, declared_size).await?;
                let blob =
                    create_s3_nondedup_blob(&txn, declared_size, policy.id, &upload_id).await?;
                let created =
                    create_new_file_from_blob(&txn, scope, folder_id, &filename, &blob, now)
                        .await?;
                update_storage_used(&txn, scope, declared_size).await?;
                txn.commit().await.map_err(AsterError::from)?;
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

    // relay_stream 的真正无暂存 fast path 需要先知道文件大小，避免在未解析策略前就开始写远端对象。
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
                scope,
                payload,
                effective_folder_id,
                relative_path,
                &resolved_filename,
                &policy,
                declared_size,
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
                scope,
                payload,
                effective_folder_id,
                relative_path,
                &resolved_filename,
                &policy,
                declared_size,
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
    let temp_path =
        crate::utils::paths::temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(temp_dir)
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
                size += chunk.len() as i64;
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
        scope,
        effective_folder_id,
        &filename,
        &temp_path,
        size,
        None,
        false,
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

pub(crate) async fn create_empty(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
) -> Result<file::Model> {
    tracing::debug!(
        scope = ?scope,
        folder_id,
        filename = %filename,
        "creating empty file"
    );

    if let Some(folder_id) = folder_id {
        verify_folder_access(state, scope, folder_id).await?;
    }
    crate::utils::validate_name(filename)?;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const EMPTY_SIZE: i64 = 0;

    let policy = resolve_policy_for_size(state, scope, folder_id, EMPTY_SIZE).await?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let should_dedup = local_content_dedup_enabled(&policy);
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let blob = if should_dedup {
        let storage_path = crate::utils::storage_path_from_hash(EMPTY_SHA256);
        let blob = file_repo::find_or_create_blob(
            &txn,
            EMPTY_SHA256,
            EMPTY_SIZE,
            policy.id,
            &storage_path,
        )
        .await?;
        if blob.inserted {
            driver.put(&storage_path, &[]).await?;
        }
        blob.model
    } else if policy.driver_type == crate::types::DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        let blob = create_s3_nondedup_blob(&txn, EMPTY_SIZE, policy.id, &upload_id).await?;
        driver.put(&blob.storage_path, &[]).await?;
        blob
    } else {
        let blob = create_nondedup_blob(&txn, EMPTY_SIZE, policy.id).await?;
        driver.put(&blob.storage_path, &[]).await?;
        blob
    };

    let created = create_new_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?;
    txn.commit().await.map_err(AsterError::from)?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FileCreated,
            scope,
            vec![created.id],
            vec![],
            vec![created.folder_id],
        ),
    );
    tracing::debug!(
        scope = ?scope,
        file_id = created.id,
        blob_id = created.blob_id,
        folder_id = created.folder_id,
        "created empty file"
    );
    Ok(created)
}
