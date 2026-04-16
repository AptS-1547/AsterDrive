use chrono::Utc;
use sea_orm::TransactionTrait;

use crate::db::repository::{file_repo, upload_session_part_repo, upload_session_repo};
use crate::entities::{file, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::upload_service::scope::{load_upload_session, personal_scope, team_scope};
use crate::services::upload_service::shared::{
    find_file_by_session, mark_session_failed, transition_upload_session_to_assembling,
};
use crate::services::{
    workspace_models::FileInfo,
    workspace_storage_service::{self},
};
use crate::storage::driver::StorageDriver;
use crate::types::{DriverType, UploadSessionStatus};
use crate::utils::paths;

/// 完成分片上传：组装 → 按策略决定是否计算 hash / 去重 → 写入最终存储
async fn complete_upload_impl(
    state: &AppState,
    session: upload_session::Model,
    parts: Option<Vec<(i32, String)>>,
) -> Result<file::Model> {
    let db = &state.db;
    let upload_id = session.id.as_str();
    tracing::debug!(
        upload_id,
        status = ?session.status,
        received_count = session.received_count,
        total_chunks = session.total_chunks,
        has_parts = parts.as_ref().is_some_and(|items| !items.is_empty()),
        "completing upload session"
    );

    if session.status == UploadSessionStatus::Completed {
        return find_file_by_session(db, &session).await;
    }

    if session.status == UploadSessionStatus::Assembling {
        return Err(AsterError::upload_assembling(
            "upload is being processed, please wait and retry in a few seconds",
        ));
    }

    if session.status == UploadSessionStatus::Failed {
        return Err(AsterError::upload_assembly_failed(
            "upload assembly failed previously; please start a new upload",
        ));
    }

    if session.status == UploadSessionStatus::Presigned {
        if session.s3_multipart_id.is_some() {
            let parts = parts.ok_or_else(|| {
                AsterError::validation_error("parts required for multipart upload completion")
            })?;
            return complete_s3_multipart(state, session, parts).await;
        }
        return complete_presigned_upload(state, session).await;
    }

    if session.status == UploadSessionStatus::Uploading && session.s3_multipart_id.is_some() {
        return complete_s3_relay_multipart(state, session).await;
    }

    if session.received_count != session.total_chunks {
        return Err(AsterError::upload_assembly_failed(format!(
            "expected {} chunks, got {}",
            session.total_chunks, session.received_count
        )));
    }

    let transitioned = upload_session_repo::try_transition_status(
        db,
        upload_id,
        UploadSessionStatus::Uploading,
        UploadSessionStatus::Assembling,
    )
    .await?;
    if !transitioned {
        return Err(AsterError::upload_assembly_failed(format!(
            "session status is '{:?}', expected 'uploading'",
            session.status
        )));
    }

    let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let should_dedup = workspace_storage_service::local_content_dedup_enabled(&policy);

    let result = async {
        use sha2::{Digest, Sha256};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        const ASSEMBLY_BUFFER_SIZE: usize = 64 * 1024;

        let assembled_path =
            paths::upload_assembled_path(&state.config.server.upload_temp_dir, upload_id);
        let mut out_file = tokio::fs::File::create(&assembled_path)
            .await
            .map_aster_err_ctx("create assembled file", AsterError::upload_assembly_failed)?;
        let mut hasher = should_dedup.then(Sha256::new);
        let mut size: i64 = 0;
        let mut buffer = vec![0u8; ASSEMBLY_BUFFER_SIZE];

        for i in 0..session.total_chunks {
            let chunk_path =
                paths::upload_chunk_path(&state.config.server.upload_temp_dir, upload_id, i);
            let mut chunk_file = tokio::fs::File::open(&chunk_path).await.map_aster_err_ctx(
                &format!("open chunk {i}"),
                AsterError::upload_assembly_failed,
            )?;

            loop {
                let n = chunk_file.read(&mut buffer).await.map_aster_err_ctx(
                    &format!("read chunk {i}"),
                    AsterError::upload_assembly_failed,
                )?;
                if n == 0 {
                    break;
                }

                let data = &buffer[..n];
                if let Some(hasher) = hasher.as_mut() {
                    hasher.update(data);
                }
                size += n as i64;
                out_file
                    .write_all(data)
                    .await
                    .map_aster_err_ctx("write assembled", AsterError::upload_assembly_failed)?;
            }
        }
        out_file
            .flush()
            .await
            .map_aster_err_ctx("flush assembled", AsterError::upload_assembly_failed)?;
        drop(out_file);

        let now = Utc::now();
        let preuploaded_blob = if hasher.is_none() {
            Some(workspace_storage_service::prepare_non_dedup_blob_upload(
                &policy, size,
            ))
        } else {
            None
        };

        if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
            workspace_storage_service::upload_temp_file_to_prepared_blob(
                driver.as_ref(),
                preuploaded_blob,
                &assembled_path,
            )
            .await?;
        }

        let create_result = async {
            let txn = state.db.begin().await.map_err(AsterError::from)?;

            let blob = if let Some(hasher) = hasher {
                let file_hash = crate::utils::hash::sha256_digest_to_hex(&hasher.finalize());
                let storage_path = crate::utils::storage_path_from_hash(&file_hash);
                let blob = file_repo::find_or_create_blob(
                    &txn,
                    &file_hash,
                    size,
                    policy.id,
                    &storage_path,
                )
                .await?;
                if blob.inserted {
                    driver.put_file(&storage_path, &assembled_path).await?;
                } else {
                    crate::utils::cleanup_temp_file(&assembled_path).await;
                }
                blob.model
            } else if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
                workspace_storage_service::persist_preuploaded_blob(&txn, preuploaded_blob).await?
            } else if policy.driver_type == DriverType::S3 {
                let blob = workspace_storage_service::create_s3_nondedup_blob(
                    &txn, size, policy.id, upload_id,
                )
                .await?;
                driver.put_file(&blob.storage_path, &assembled_path).await?;
                blob
            } else {
                let blob =
                    workspace_storage_service::create_nondedup_blob(&txn, size, policy.id).await?;
                driver.put_file(&blob.storage_path, &assembled_path).await?;
                blob
            };

            let created =
                workspace_storage_service::finalize_upload_session_blob(&txn, &session, &blob, now)
                    .await?;

            txn.commit().await.map_err(AsterError::from)?;
            Ok::<file::Model, AsterError>(created)
        }
        .await;

        match create_result {
            Ok(created) => Ok(created),
            Err(error) => {
                if let Some(preuploaded_blob) = preuploaded_blob.as_ref() {
                    workspace_storage_service::cleanup_preuploaded_blob_upload(
                        driver.as_ref(),
                        preuploaded_blob,
                        "chunked upload DB error after storing assembled blob",
                    )
                    .await;
                }
                Err(error)
            }
        }
    }
    .await;

    match result {
        Ok(created) => {
            let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, upload_id);
            crate::utils::cleanup_temp_dir(&temp_dir).await;
            tracing::debug!(
                upload_id,
                file_id = created.id,
                blob_id = created.blob_id,
                size = created.size,
                "completed upload session"
            );
            Ok(created)
        }
        Err(error) => {
            mark_session_failed(db, upload_id).await;
            Err(error)
        }
    }
}

pub async fn complete_upload(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
    parts: Option<Vec<(i32, String)>>,
) -> Result<FileInfo> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    complete_upload_impl(state, session, parts)
        .await
        .map(FileInfo::from)
}

pub async fn complete_upload_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
    parts: Option<Vec<(i32, String)>>,
) -> Result<FileInfo> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    complete_upload_impl(state, session, parts)
        .await
        .map(FileInfo::from)
}

async fn ensure_uploaded_s3_object_size(
    driver: &dyn StorageDriver,
    temp_key: &str,
    declared_size: i64,
    missing_message: &str,
) -> Result<i64> {
    let meta = driver
        .metadata(temp_key)
        .await
        .map_aster_err_with(|| AsterError::upload_assembly_failed(missing_message))?;
    let actual_size = meta.size as i64;

    if actual_size != declared_size {
        if let Err(error) = driver.delete(temp_key).await {
            tracing::warn!("failed to delete S3 temp object: {error}");
        }
        return Err(AsterError::upload_assembly_failed(format!(
            "size mismatch: declared {} but uploaded {}",
            declared_size, actual_size
        )));
    }

    Ok(actual_size)
}

async fn finalize_s3_upload_session(
    state: &AppState,
    session: &upload_session::Model,
    policy_id: i64,
    storage_path: &str,
    size: i64,
) -> Result<file::Model> {
    workspace_storage_service::finalize_upload_session_file(
        state,
        session,
        &format!("s3-{}", session.id),
        size,
        policy_id,
        storage_path,
        Utc::now(),
    )
    .await
}

async fn complete_s3_multipart_upload_session(
    state: &AppState,
    session: upload_session::Model,
    expected_status: UploadSessionStatus,
    mut completed_parts: Vec<(i32, String)>,
    missing_message: &str,
) -> Result<file::Model> {
    let db = &state.db;
    let temp_key = session
        .s3_temp_key
        .as_deref()
        .ok_or_else(|| AsterError::upload_assembly_failed("missing s3_temp_key"))?
        .to_string();
    let multipart_id = session
        .s3_multipart_id
        .as_deref()
        .ok_or_else(|| AsterError::upload_assembly_failed("missing s3_multipart_id"))?
        .to_string();

    let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let upload_id = session.id.clone();

    tracing::debug!(
        upload_id = %upload_id,
        status = ?session.status,
        expected_status = ?expected_status,
        policy_id = policy.id,
        part_count = completed_parts.len(),
        "completing S3 multipart upload session"
    );

    transition_upload_session_to_assembling(db, &upload_id, session.status, expected_status)
        .await?;

    let result = async {
        completed_parts.sort_by_key(|(part_number, _)| *part_number);
        driver
            .complete_multipart_upload(&temp_key, &multipart_id, completed_parts)
            .await?;

        let actual_size = ensure_uploaded_s3_object_size(
            driver.as_ref(),
            &temp_key,
            session.total_size,
            missing_message,
        )
        .await?;

        finalize_s3_upload_session(state, &session, policy.id, &temp_key, actual_size).await
    }
    .await;

    match result {
        Ok(file) => {
            tracing::debug!(
                upload_id = %upload_id,
                file_id = file.id,
                blob_id = file.blob_id,
                size = file.size,
                "completed S3 multipart upload session"
            );
            Ok(file)
        }
        Err(error) => {
            mark_session_failed(db, &upload_id).await;
            Err(error)
        }
    }
}

/// 完成 presigned 上传：校验 S3 临时对象 → 直接建文件记录
async fn complete_presigned_upload(
    state: &AppState,
    session: upload_session::Model,
) -> Result<file::Model> {
    let db = &state.db;
    let temp_key = session
        .s3_temp_key
        .as_deref()
        .ok_or_else(|| AsterError::upload_assembly_failed("missing s3_temp_key"))?
        .to_string();

    let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let actual_size = ensure_uploaded_s3_object_size(
        driver.as_ref(),
        &temp_key,
        session.total_size,
        "S3 temp object not found - upload may not have completed",
    )
    .await?;

    let upload_id = session.id.clone();
    tracing::debug!(
        upload_id = %upload_id,
        status = ?session.status,
        policy_id = policy.id,
        "completing presigned upload session"
    );
    transition_upload_session_to_assembling(
        db,
        &upload_id,
        session.status,
        UploadSessionStatus::Presigned,
    )
    .await?;

    let result = async {
        finalize_s3_upload_session(state, &session, policy.id, &temp_key, actual_size).await
    }
    .await;

    match result {
        Ok(file) => {
            tracing::debug!(
                upload_id = %upload_id,
                file_id = file.id,
                blob_id = file.blob_id,
                size = file.size,
                "completed presigned upload session"
            );
            Ok(file)
        }
        Err(error) => {
            mark_session_failed(db, &upload_id).await;
            Err(error)
        }
    }
}

/// 完成 S3 multipart presigned 上传：complete multipart → 直接建文件记录
async fn complete_s3_multipart(
    state: &AppState,
    session: upload_session::Model,
    parts: Vec<(i32, String)>,
) -> Result<file::Model> {
    complete_s3_multipart_upload_session(
        state,
        session,
        UploadSessionStatus::Presigned,
        parts,
        "S3 object not found after multipart complete - assembly may have failed",
    )
    .await
}

/// 完成 S3 relay multipart 上传：直接使用服务端保存的 parts 完成 multipart。
async fn complete_s3_relay_multipart(
    state: &AppState,
    session: upload_session::Model,
) -> Result<file::Model> {
    let db = &state.db;
    let parts = upload_session_part_repo::list_by_upload(db, &session.id).await?;
    let expected_parts =
        crate::utils::numbers::i32_to_usize(session.total_chunks, "upload session total_chunks")?;
    if parts.len() != expected_parts {
        return Err(AsterError::upload_assembly_failed(format!(
            "expected {} parts, got {}",
            session.total_chunks,
            parts.len()
        )));
    }

    for (expected, part) in (1..=session.total_chunks).zip(parts.iter()) {
        if part.part_number != expected {
            return Err(AsterError::upload_assembly_failed(format!(
                "missing uploaded part {}; got {:?}",
                expected, part.part_number
            )));
        }
    }

    let completed_parts = parts
        .into_iter()
        .map(|part| (part.part_number, part.etag))
        .collect();
    complete_s3_multipart_upload_session(
        state,
        session,
        UploadSessionStatus::Uploading,
        completed_parts,
        "S3 object not found after relay multipart complete - assembly may have failed",
    )
    .await
}
