use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use sha2::{Digest, Sha256};

use crate::db::repository::file_repo;
use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::storage_change_service;

use super::{
    HASH_BUF_SIZE, NewFileMode, PreparedNonDedupBlobUpload, WorkspaceStorageScope, check_quota,
    cleanup_preuploaded_blob_upload, create_exact_file_from_blob, create_new_file_from_blob,
    create_nondedup_blob, create_s3_nondedup_blob, local_content_dedup_enabled,
    persist_preuploaded_blob, prepare_non_dedup_blob_upload, resolve_policy_for_size,
    update_storage_used, upload_temp_file_to_prepared_blob, verify_file_access,
    verify_folder_access,
};

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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn store_preuploaded_nondedup(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
    policy: &crate::entities::storage_policy::Model,
    preuploaded_blob: PreparedNonDedupBlobUpload,
) -> Result<file::Model> {
    let db = &state.db;

    tracing::debug!(
        scope = ?scope,
        folder_id,
        filename = %filename,
        size,
        existing_file_id,
        skip_lock_check,
        policy_id = policy.id,
        "storing file from preuploaded blob"
    );

    crate::utils::validate_name(filename)?;

    let driver = state.driver_registry.get_driver(policy)?;

    if policy.max_file_size > 0 && size > policy.max_file_size {
        cleanup_preuploaded_blob_upload(
            driver.as_ref(),
            &preuploaded_blob,
            "size validation failure",
        )
        .await;
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            size, policy.max_file_size
        )));
    }

    let now = Utc::now();

    let overwrite_ctx = if let Some(existing_id) = existing_file_id {
        let old_file = verify_file_access(state, scope, existing_id).await?;
        if old_file.is_locked && !skip_lock_check {
            cleanup_preuploaded_blob_upload(
                driver.as_ref(),
                &preuploaded_blob,
                "lock check failure",
            )
            .await;
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
    let storage_delta = overwrite_ctx.as_ref().map_or(size, |_| size);

    if storage_delta > 0 {
        check_quota(db, scope, storage_delta).await?;
    }

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    let create_result = async {
        let txn = state.db.begin().await.map_err(AsterError::from)?;
        if storage_delta > 0 {
            check_quota(&txn, scope, storage_delta).await?;
        }

        let blob = persist_preuploaded_blob(&txn, &preuploaded_blob).await?;

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
            let created =
                create_new_file_from_blob(&txn, scope, folder_id, filename, &blob, now).await?;
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
            cleanup_preuploaded_blob_upload(
                driver.as_ref(),
                &preuploaded_blob,
                "DB error after direct upload",
            )
            .await;
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
        "stored file from preuploaded blob"
    );

    Ok(result)
}
