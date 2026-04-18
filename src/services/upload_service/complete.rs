//! 上传完成阶段。
//!
//! 这里把各种“临时上传状态”收口成正式文件：
//! - 本地 chunk 文件组装
//! - presigned 单文件确认
//! - presigned multipart 完成
//! - relay multipart 完成
//!
//! 目标都是在最后统一落到 `workspace_storage_service` 的文件创建语义上。

mod chunked;

use chrono::Utc;

use crate::db::repository::upload_session_part_repo;
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
use crate::types::UploadSessionStatus;
use crate::utils::numbers::u64_to_i64;

use self::chunked::complete_chunked_upload;

enum CompletionPlan {
    ReturnCompleted,
    CompletePresigned,
    CompletePresignedMultipart { parts: Vec<(i32, String)> },
    CompleteRelayMultipart,
    AssembleChunks,
}

/// 完成分片上传：组装 → 按策略决定是否计算 hash / 去重 → 写入最终存储
async fn complete_upload_impl(
    state: &AppState,
    session: upload_session::Model,
    parts: Option<Vec<(i32, String)>>,
) -> Result<file::Model> {
    tracing::debug!(
        upload_id = %session.id,
        status = ?session.status,
        received_count = session.received_count,
        total_chunks = session.total_chunks,
        has_parts = parts.as_ref().is_some_and(|items| !items.is_empty()),
        "completing upload session"
    );

    match determine_completion_plan(&session, parts)? {
        CompletionPlan::ReturnCompleted => find_file_by_session(&state.db, &session).await,
        CompletionPlan::CompletePresigned => complete_presigned_upload(state, session).await,
        CompletionPlan::CompletePresignedMultipart { parts } => {
            complete_s3_multipart(state, session, parts).await
        }
        CompletionPlan::CompleteRelayMultipart => complete_s3_relay_multipart(state, session).await,
        CompletionPlan::AssembleChunks => complete_chunked_upload(state, session).await,
    }
}

fn determine_completion_plan(
    session: &upload_session::Model,
    parts: Option<Vec<(i32, String)>>,
) -> Result<CompletionPlan> {
    if session.status == UploadSessionStatus::Completed {
        return Ok(CompletionPlan::ReturnCompleted);
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
            return Ok(CompletionPlan::CompletePresignedMultipart { parts });
        }

        // presigned 单文件没有分片清单，只需要校验 temp object 真实存在且大小匹配。
        return Ok(CompletionPlan::CompletePresigned);
    }

    if session.status == UploadSessionStatus::Uploading && session.s3_multipart_id.is_some() {
        // relay multipart 的 completed parts 由服务端在 chunk 阶段自行收集，
        // complete 时无需客户端再次回传。
        return Ok(CompletionPlan::CompleteRelayMultipart);
    }

    if session.received_count != session.total_chunks {
        return Err(AsterError::upload_assembly_failed(format!(
            "expected {} chunks, got {}",
            session.total_chunks, session.received_count
        )));
    }

    Ok(CompletionPlan::AssembleChunks)
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
    let actual_size = u64_to_i64(meta.size, "blob_size")?;

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
    // S3 直传模式不会经过本地 assembled 文件，complete 阶段只负责把已经存在的对象
    // 记成 blob + file，并原子更新配额和 session 状态。
    workspace_storage_service::finalize_upload_session_file(
        state,
        workspace_storage_service::FinalizeUploadSessionFileParams {
            session,
            file_hash: &format!("s3-{}", session.id),
            size,
            policy_id,
            storage_path,
            now: Utc::now(),
        },
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
    let multipart = state.driver_registry.get_multipart_driver(&policy)?;
    let driver_ref: &dyn StorageDriver = driver.as_ref();
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
        // multipart complete 之前要先把 part 列表排序；驱动层依赖有序 part 序列。
        multipart
            .complete_multipart_upload(&temp_key, &multipart_id, completed_parts)
            .await?;

        let actual_size = ensure_uploaded_s3_object_size(
            driver_ref,
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
    // presigned 单文件的 complete 阶段，本质是“确认对象存在且大小正确”，
    // 然后把 temp_key 直接认领成正式 blob。
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
