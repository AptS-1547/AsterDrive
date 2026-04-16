use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, Set};

use crate::db::repository::{file_repo, upload_session_repo};
use crate::entities::{file, upload_session};
use crate::errors::{AsterError, Result};
use crate::types::UploadSessionStatus;
use crate::utils::id;

pub(super) fn upload_session_chunk_unavailable_error(
    session: &upload_session::Model,
) -> AsterError {
    match session.status {
        UploadSessionStatus::Failed => {
            AsterError::upload_session_expired("session was canceled or failed")
        }
        UploadSessionStatus::Assembling => {
            AsterError::upload_session_expired("session is assembling and no longer accepts chunks")
        }
        UploadSessionStatus::Completed => {
            AsterError::upload_session_expired("session already completed")
        }
        UploadSessionStatus::Presigned => {
            AsterError::validation_error("session does not accept relay chunk uploads")
        }
        UploadSessionStatus::Uploading => {
            AsterError::upload_session_not_found(format!("session {}", session.id))
        }
    }
}

pub(super) fn expected_chunk_size_for_upload(
    session: &upload_session::Model,
    chunk_number: i32,
) -> Result<i64> {
    if session.total_chunks <= 0 || session.chunk_size <= 0 {
        return Err(AsterError::chunk_upload_failed(format!(
            "invalid upload session chunk metadata: total_chunks={}, chunk_size={}",
            session.total_chunks, session.chunk_size
        )));
    }

    if chunk_number < session.total_chunks - 1 {
        return Ok(session.chunk_size);
    }

    let preceding = session.chunk_size * i64::from(session.total_chunks - 1);
    let expected = session.total_size - preceding;
    if expected <= 0 {
        return Err(AsterError::chunk_upload_failed(format!(
            "invalid final chunk size for upload {}: total_size={}, preceding={preceding}",
            session.id, session.total_size
        )));
    }
    Ok(expected)
}

/// 生成唯一的 upload_id（UUID v4），最多重试 5 次防止极低概率碰撞
pub(super) async fn generate_upload_id<C: ConnectionTrait>(db: &C) -> Result<String> {
    for _ in 0..5 {
        let candidate = id::new_uuid();
        match upload_session_repo::find_by_id(db, &candidate).await {
            Err(e) if e.code() == "E054" => return Ok(candidate),
            Err(e) => return Err(e),
            Ok(_) => {
                tracing::warn!("upload_id collision: {candidate}, retrying");
                continue;
            }
        }
    }
    Err(AsterError::internal_error(
        "failed to generate unique upload_id after 5 attempts",
    ))
}

pub(super) fn upload_session_status_label(status: UploadSessionStatus) -> &'static str {
    match status {
        UploadSessionStatus::Uploading => "uploading",
        UploadSessionStatus::Assembling => "assembling",
        UploadSessionStatus::Completed => "completed",
        UploadSessionStatus::Failed => "failed",
        UploadSessionStatus::Presigned => "presigned",
    }
}

pub(super) async fn transition_upload_session_to_assembling<C: ConnectionTrait>(
    db: &C,
    upload_id: &str,
    actual_status: UploadSessionStatus,
    expected_status: UploadSessionStatus,
) -> Result<()> {
    let transitioned = upload_session_repo::try_transition_status(
        db,
        upload_id,
        expected_status,
        UploadSessionStatus::Assembling,
    )
    .await?;
    if !transitioned {
        return Err(AsterError::upload_assembly_failed(format!(
            "session status is '{:?}', expected '{}'",
            actual_status,
            upload_session_status_label(expected_status)
        )));
    }
    Ok(())
}

/// 根据 session 查找已完成的文件（幂等重试用）
pub(super) async fn find_file_by_session<C: ConnectionTrait>(
    db: &C,
    session: &upload_session::Model,
) -> Result<file::Model> {
    let file_id = session.file_id.ok_or_else(|| {
        AsterError::upload_assembly_failed(
            "upload already completed but file_id not found; please refresh",
        )
    })?;
    file_repo::find_by_id(db, file_id).await
}

/// 将 session 标记为 Failed（best-effort，失败只记录日志）
pub(super) async fn mark_session_failed<C: ConnectionTrait>(db: &C, upload_id: &str) {
    if let Ok(session) = upload_session_repo::find_by_id(db, upload_id).await {
        let mut active: upload_session::ActiveModel = session.into();
        active.status = Set(UploadSessionStatus::Failed);
        active.updated_at = Set(Utc::now());
        if let Err(error) = upload_session_repo::update(db, active).await {
            tracing::warn!("failed to mark session {upload_id} as failed: {error}");
        }
    }
}

pub(super) async fn mark_session_failed_with_expiration<C: ConnectionTrait>(
    db: &C,
    upload_id: &str,
    expires_at: DateTime<Utc>,
) -> Result<()> {
    let session = upload_session_repo::find_by_id(db, upload_id).await?;
    let mut active: upload_session::ActiveModel = session.into();
    active.status = Set(UploadSessionStatus::Failed);
    active.expires_at = Set(expires_at);
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(db, active).await?;
    Ok(())
}
