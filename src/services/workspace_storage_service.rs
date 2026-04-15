use actix_multipart::Multipart;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set, TransactionTrait};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
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
    create_nondedup_blob_with_key, create_s3_nondedup_blob, ensure_upload_parent_path,
    finalize_upload_session_blob, finalize_upload_session_file, load_storage_limits,
    local_content_dedup_enabled, parse_relative_upload_path, resolve_policy_for_size,
    update_storage_used,
};

const HASH_BUF_SIZE: usize = 65536;

#[derive(Clone, Copy)]
enum NewFileMode {
    ResolveUnique,
    Exact,
}

#[derive(Debug, Clone)]
pub(crate) enum PreparedNonDedupBlobUpload {
    Local {
        base_path: PathBuf,
        blob_key: String,
        storage_path: String,
        size: i64,
        policy_id: i64,
    },
    S3 {
        upload_id: String,
        storage_path: String,
        size: i64,
        policy_id: i64,
    },
}

impl PreparedNonDedupBlobUpload {
    fn storage_path(&self) -> &str {
        match self {
            Self::Local { storage_path, .. } | Self::S3 { storage_path, .. } => storage_path,
        }
    }
}

pub(crate) fn prepare_non_dedup_blob_upload(
    policy: &crate::entities::storage_policy::Model,
    size: i64,
) -> PreparedNonDedupBlobUpload {
    if policy.driver_type == DriverType::S3 {
        let upload_id = crate::utils::id::new_uuid();
        PreparedNonDedupBlobUpload::S3 {
            storage_path: format!("files/{upload_id}"),
            upload_id,
            size,
            policy_id: policy.id,
        }
    } else {
        let blob_key = crate::utils::id::new_short_token();
        PreparedNonDedupBlobUpload::Local {
            base_path: crate::storage::local::effective_base_path(policy),
            storage_path: crate::utils::storage_path_from_blob_key(&blob_key),
            blob_key,
            size,
            policy_id: policy.id,
        }
    }
}

fn normalize_absolute_cleanup_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    Some(normalized)
}

fn normalize_cleanup_root(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return normalize_absolute_cleanup_path(path);
    }

    let current_dir = std::env::current_dir().ok()?;
    normalize_absolute_cleanup_path(&current_dir.join(path))
}

async fn cleanup_empty_local_blob_dirs(prefix_dir: &Path, root_dir: &Path) {
    let Some(mut current) = normalize_cleanup_root(prefix_dir) else {
        tracing::warn!(
            "skip blob dir cleanup for unresolved prefix {}",
            prefix_dir.display()
        );
        return;
    };
    let Some(root_dir) = normalize_cleanup_root(root_dir) else {
        tracing::warn!(
            "skip blob dir cleanup for unresolved root {}",
            root_dir.display()
        );
        return;
    };

    if current == root_dir || !current.starts_with(&root_dir) {
        tracing::warn!(
            "skip blob dir cleanup outside storage root: prefix={}, root={}",
            current.display(),
            root_dir.display()
        );
        return;
    }

    while current != root_dir {
        match tokio::fs::remove_dir(&current).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) => {
                tracing::warn!("failed to cleanup blob dir {}: {error}", current.display());
                break;
            }
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

pub(crate) async fn cleanup_preuploaded_blob_upload(
    driver: &dyn crate::storage::driver::StorageDriver,
    prepared: &PreparedNonDedupBlobUpload,
    reason: &str,
) {
    match prepared {
        PreparedNonDedupBlobUpload::Local {
            base_path,
            storage_path,
            ..
        } => {
            let full_path = base_path.join(storage_path.trim_start_matches('/'));
            match tokio::fs::remove_file(&full_path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(
                        storage_path = %storage_path,
                        full_path = %full_path.display(),
                        "failed to cleanup preuploaded local blob after {reason}: {error}"
                    );
                    return;
                }
            }

            if let Some(parent) = full_path.parent() {
                cleanup_empty_local_blob_dirs(parent, base_path).await;
            }
        }
        PreparedNonDedupBlobUpload::S3 { .. } => {
            if let Err(cleanup_err) = driver.delete(prepared.storage_path()).await {
                tracing::warn!(
                    storage_path = %prepared.storage_path(),
                    "failed to cleanup preuploaded blob after {reason}: {cleanup_err}"
                );
            }
        }
    }
}

pub(crate) async fn upload_temp_file_to_prepared_blob(
    driver: &dyn crate::storage::driver::StorageDriver,
    prepared: &PreparedNonDedupBlobUpload,
    temp_path: &str,
) -> Result<()> {
    if let Err(error) = driver.put_file(prepared.storage_path(), temp_path).await {
        cleanup_preuploaded_blob_upload(driver, prepared, "upload error").await;
        return Err(error);
    }

    Ok(())
}

pub(crate) async fn persist_preuploaded_blob<C: ConnectionTrait>(
    db: &C,
    prepared: &PreparedNonDedupBlobUpload,
) -> Result<file_blob::Model> {
    match prepared {
        PreparedNonDedupBlobUpload::Local {
            blob_key,
            storage_path,
            size,
            policy_id,
            ..
        } => create_nondedup_blob_with_key(db, *size, *policy_id, blob_key, storage_path).await,
        PreparedNonDedupBlobUpload::S3 {
            upload_id,
            size,
            policy_id,
            ..
        } => create_s3_nondedup_blob(db, *size, *policy_id, upload_id).await,
    }
}

fn relay_stream_direct_upload_eligible(
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

    let preuploaded_blob = if dedup_target.is_none() {
        Some(prepare_non_dedup_blob_upload(&policy, size))
    } else {
        None
    };

    if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
        upload_temp_file_to_prepared_blob(driver.as_ref(), preuploaded_blob, temp_path).await?;
    }

    let create_result = async {
        let txn = state.db.begin().await.map_err(AsterError::from)?;
        if storage_delta > 0 {
            check_quota(&txn, scope, storage_delta).await?;
        }

        let blob = if let Some((file_hash, storage_path)) = dedup_target.as_ref() {
            let blob =
                file_repo::find_or_create_blob(&txn, file_hash, size, policy.id, storage_path)
                    .await?;
            if blob.inserted {
                driver.put_file(storage_path, temp_path).await?;
            }
            blob.model
        } else if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
            persist_preuploaded_blob(&txn, preuploaded_blob).await?
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

            let next_ver =
                crate::db::repository::version_repo::next_version(&txn, existing_id).await?;
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
                    create_exact_file_from_blob(&txn, scope, folder_id, filename, &blob, now)
                        .await?
                }
            };
            if storage_delta != 0 {
                update_storage_used(&txn, scope, storage_delta).await?;
            }
            created
        };

        txn.commit().await.map_err(AsterError::from)?;
        Ok::<file::Model, AsterError>(result)
    }
    .await;

    let result = match create_result {
        Ok(result) => result,
        Err(error) => {
            if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
                cleanup_preuploaded_blob_upload(
                    driver.as_ref(),
                    preuploaded_blob,
                    "DB error after temp file upload",
                )
                .await;
            }
            return Err(error);
        }
    };

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

#[cfg(test)]
mod tests {
    use super::{
        WorkspaceStorageScope, store_from_temp_exact_name_with_hints, store_from_temp_with_hints,
    };
    use crate::cache;
    use crate::config::{CacheConfig, Config, DatabaseConfig, RuntimeConfig};
    use crate::entities::{file_blob, storage_policy, user};
    use crate::runtime::AppState;
    use crate::services::mail_service;
    use crate::storage::driver::{BlobMetadata, StoragePathVisitor};
    use crate::storage::{DriverRegistry, PolicySnapshot, StorageDriver};
    use crate::types::{DriverType, UserRole, UserStatus};
    use async_trait::async_trait;
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::AsyncRead;
    use tokio::sync::{Notify, oneshot};

    struct BlockingPutFileDriver {
        inner: crate::storage::local::LocalDriver,
        put_file_entered: Mutex<Option<oneshot::Sender<()>>>,
        release_put_file: Arc<Notify>,
    }

    impl BlockingPutFileDriver {
        fn new(policy: &storage_policy::Model) -> (Self, oneshot::Receiver<()>, Arc<Notify>) {
            let (entered_tx, entered_rx) = oneshot::channel();
            let release_put_file = Arc::new(Notify::new());
            (
                Self {
                    inner: crate::storage::local::LocalDriver::new(policy)
                        .expect("blocking test driver should initialize"),
                    put_file_entered: Mutex::new(Some(entered_tx)),
                    release_put_file: release_put_file.clone(),
                },
                entered_rx,
                release_put_file,
            )
        }
    }

    #[async_trait]
    impl StorageDriver for BlockingPutFileDriver {
        async fn put(&self, path: &str, data: &[u8]) -> crate::errors::Result<String> {
            self.inner.put(path, data).await
        }

        async fn get(&self, path: &str) -> crate::errors::Result<Vec<u8>> {
            self.inner.get(path).await
        }

        async fn get_stream(
            &self,
            path: &str,
        ) -> crate::errors::Result<Box<dyn AsyncRead + Unpin + Send>> {
            self.inner.get_stream(path).await
        }

        async fn delete(&self, path: &str) -> crate::errors::Result<()> {
            self.inner.delete(path).await
        }

        async fn exists(&self, path: &str) -> crate::errors::Result<bool> {
            self.inner.exists(path).await
        }

        async fn metadata(&self, path: &str) -> crate::errors::Result<BlobMetadata> {
            self.inner.metadata(path).await
        }

        async fn list_paths(&self, prefix: Option<&str>) -> crate::errors::Result<Vec<String>> {
            self.inner.list_paths(prefix).await
        }

        async fn scan_paths(
            &self,
            prefix: Option<&str>,
            visitor: &mut dyn StoragePathVisitor,
        ) -> crate::errors::Result<()> {
            self.inner.scan_paths(prefix, visitor).await
        }

        async fn put_file(
            &self,
            storage_path: &str,
            local_path: &str,
        ) -> crate::errors::Result<String> {
            if let Some(sender) = self
                .put_file_entered
                .lock()
                .expect("blocking test driver lock should succeed")
                .take()
            {
                let _ = sender.send(());
            }
            self.release_put_file.notified().await;
            self.inner.put_file(storage_path, local_path).await
        }

        async fn presigned_url(
            &self,
            path: &str,
            expires: Duration,
        ) -> crate::errors::Result<Option<String>> {
            self.inner.presigned_url(path, expires).await
        }
    }

    fn snapshot_dir_tree(path: &Path) -> std::io::Result<BTreeSet<String>> {
        fn walk(
            root: &Path,
            current: &Path,
            entries: &mut BTreeSet<String>,
        ) -> std::io::Result<()> {
            for entry in std::fs::read_dir(current)? {
                let entry = entry?;
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    entries.insert(format!("{relative}/"));
                    walk(root, &path, entries)?;
                } else {
                    entries.insert(relative);
                }
            }
            Ok(())
        }

        let mut entries = BTreeSet::new();
        if !path.exists() {
            return Ok(entries);
        }
        walk(path, path, &mut entries)?;
        Ok(entries)
    }

    async fn build_test_state() -> (AppState, PathBuf, storage_policy::Model, user::Model) {
        let temp_root = std::env::temp_dir().join(format!(
            "asterdrive-workspace-storage-service-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).expect("temp root should be created");
        let uploads_root = temp_root.join("uploads");
        std::fs::create_dir_all(&uploads_root).expect("uploads root should be created");

        let db = crate::db::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            pool_size: 1,
            retry_count: 0,
        })
        .await
        .unwrap();
        Migrator::up(&db, None).await.unwrap();

        let now = Utc::now();
        let policy = storage_policy::ActiveModel {
            name: Set("Test Local Policy".to_string()),
            driver_type: Set(DriverType::Local),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set(uploads_root.to_string_lossy().into_owned()),
            max_file_size: Set(0),
            allowed_types: Set(crate::types::StoredStoragePolicyAllowedTypes::empty()),
            options: Set(crate::types::StoredStoragePolicyOptions::empty()),
            is_default: Set(true),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let user = user::ActiveModel {
            username: Set("storage-conflict-user".to_string()),
            email: Set("storage-conflict@example.com".to_string()),
            password_hash: Set("not-used".to_string()),
            role: Set(UserRole::User),
            status: Set(UserStatus::Active),
            session_version: Set(0),
            email_verified_at: Set(Some(now)),
            pending_email: Set(None),
            storage_used: Set(0),
            storage_quota: Set(0),
            policy_group_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            config: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let runtime_config = Arc::new(RuntimeConfig::new());
        let cache = cache::create_cache(&CacheConfig {
            enabled: false,
            ..Default::default()
        })
        .await;
        let mut config = Config::default();
        config.server.temp_dir = temp_root.join(".tmp").to_string_lossy().into_owned();
        config.server.upload_temp_dir = temp_root.join(".uploads").to_string_lossy().into_owned();

        let (thumbnail_tx, _thumbnail_rx) = tokio::sync::mpsc::channel::<i64>(1);
        let (storage_change_tx, _) = tokio::sync::broadcast::channel(
            crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
        );

        let state = AppState {
            db,
            driver_registry: Arc::new(DriverRegistry::new()),
            runtime_config: runtime_config.clone(),
            policy_snapshot: Arc::new(PolicySnapshot::new()),
            config: Arc::new(config),
            cache,
            mail_sender: mail_service::runtime_sender(runtime_config),
            thumbnail_tx,
            storage_change_tx,
        };

        (state, temp_root, policy, user)
    }

    #[tokio::test]
    async fn exact_name_conflict_cleans_preuploaded_local_blob() {
        let (state, temp_root, policy, user) = build_test_state().await;
        let scope = WorkspaceStorageScope::Personal { user_id: user.id };
        let uploads_root = temp_root.join("uploads");

        let first_temp = temp_root.join("first.bin");
        let first_bytes = b"first payload";
        tokio::fs::write(&first_temp, first_bytes).await.unwrap();
        store_from_temp_with_hints(
            &state,
            scope,
            None,
            "dup.txt",
            &first_temp.to_string_lossy(),
            first_bytes.len() as i64,
            None,
            false,
            Some(policy.clone()),
            None,
        )
        .await
        .unwrap();

        let blob_count_before = file_blob::Entity::find().count(&state.db).await.unwrap();
        let upload_tree_before = snapshot_dir_tree(&uploads_root).unwrap();

        let second_temp = temp_root.join("second.bin");
        let second_bytes = b"second payload should be cleaned";
        tokio::fs::write(&second_temp, second_bytes).await.unwrap();
        let err = store_from_temp_exact_name_with_hints(
            &state,
            scope,
            None,
            "dup.txt",
            &second_temp.to_string_lossy(),
            second_bytes.len() as i64,
            None,
            false,
            Some(policy),
            None,
        )
        .await
        .expect_err("exact-name conflict should fail");

        assert!(
            err.message().contains("already exists"),
            "unexpected error message: {}",
            err.message()
        );

        let blob_count_after = file_blob::Entity::find().count(&state.db).await.unwrap();
        let upload_tree_after = snapshot_dir_tree(&uploads_root).unwrap();
        assert_eq!(blob_count_after, blob_count_before);
        assert_eq!(upload_tree_after, upload_tree_before);

        drop(state);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn slow_nondedup_preupload_does_not_block_task_listing() {
        let (state, temp_root, policy, user) = build_test_state().await;
        let scope = WorkspaceStorageScope::Personal { user_id: user.id };
        let (blocking_driver, entered_rx, release_put_file) = BlockingPutFileDriver::new(&policy);
        state
            .driver_registry
            .insert_for_test(policy.id, Arc::new(blocking_driver));

        let temp_file = temp_root.join("slow-upload.bin");
        let payload = b"slow upload payload";
        tokio::fs::write(&temp_file, payload).await.unwrap();

        let state_for_store = state.clone();
        let policy_for_store = policy.clone();
        let temp_path = temp_file.to_string_lossy().into_owned();
        let store_task = tokio::spawn(async move {
            store_from_temp_with_hints(
                &state_for_store,
                scope,
                None,
                "slow-upload.bin",
                &temp_path,
                payload.len() as i64,
                None,
                false,
                Some(policy_for_store),
                None,
            )
            .await
        });

        tokio::time::timeout(Duration::from_secs(1), entered_rx)
            .await
            .expect("preupload should reach put_file")
            .expect("put_file entry signal should be sent");

        let page = tokio::time::timeout(
            Duration::from_millis(250),
            crate::services::task_service::list_tasks_paginated_in_scope(&state, scope, 20, 0),
        )
        .await
        .expect("task listing should not wait for blocked blob upload")
        .expect("task listing should succeed");
        assert_eq!(page.total, 0);
        assert!(page.items.is_empty());

        release_put_file.notify_one();

        let stored = tokio::time::timeout(Duration::from_secs(1), store_task)
            .await
            .expect("store task should finish after releasing upload")
            .expect("store task should join")
            .expect("store task should succeed");
        assert_eq!(stored.name, "slow-upload.bin");

        drop(state);
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
