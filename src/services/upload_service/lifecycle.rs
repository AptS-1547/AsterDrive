use chrono::{Duration, Utc};

use crate::db::repository::upload_session_repo;
use crate::entities::upload_session;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::upload_service::scope::{load_upload_session, personal_scope, team_scope};
use crate::services::upload_service::shared::mark_session_failed_with_expiration;
use crate::types::UploadSessionStatus;
use crate::utils::paths;

const CANCELED_MULTIPART_SESSION_GRACE_SECS: i64 = 15;

async fn cleanup_upload_temp_dir(state: &AppState, upload_id: &str) {
    let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, upload_id);
    crate::utils::cleanup_temp_dir(&temp_dir).await;
}

/// 取消上传
async fn cancel_upload_impl(state: &AppState, session: upload_session::Model) -> Result<()> {
    let upload_id = session.id.as_str();
    tracing::debug!(
        upload_id,
        status = ?session.status,
        policy_id = session.policy_id,
        has_temp_key = session.s3_temp_key.is_some(),
        has_multipart_id = session.s3_multipart_id.is_some(),
        "canceling upload session"
    );

    if session.s3_multipart_id.is_some()
        && matches!(
            session.status,
            UploadSessionStatus::Uploading
                | UploadSessionStatus::Presigned
                | UploadSessionStatus::Assembling
        )
    {
        let expires_at = Utc::now() + Duration::seconds(CANCELED_MULTIPART_SESSION_GRACE_SECS);
        mark_session_failed_with_expiration(&state.db, upload_id, expires_at).await?;

        cleanup_upload_temp_dir(state, upload_id).await;
        tracing::debug!(
            upload_id,
            expires_at = %expires_at,
            "deferred cleanup for canceled multipart upload session"
        );
        return Ok(());
    }

    if let Some(ref temp_key) = session.s3_temp_key {
        let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
        if let Ok(driver) = state.driver_registry.get_driver(&policy) {
            if let Some(ref multipart_id) = session.s3_multipart_id {
                if let Some(multipart) = driver.as_multipart() {
                    if let Err(error) = multipart
                        .abort_multipart_upload(temp_key, multipart_id)
                        .await
                    {
                        tracing::warn!("failed to abort S3 multipart upload: {error}");
                    }
                }
                if let Err(error) = driver.delete(temp_key).await {
                    tracing::warn!("failed to delete S3 temp object after abort: {error}");
                }
            } else if let Err(error) = driver.delete(temp_key).await {
                tracing::warn!("failed to delete S3 temp object: {error}");
            }
        }
    }

    cleanup_upload_temp_dir(state, upload_id).await;
    upload_session_repo::delete(&state.db, upload_id).await?;
    tracing::debug!(upload_id, "canceled upload session");
    Ok(())
}

pub async fn cancel_upload(state: &AppState, upload_id: &str, user_id: i64) -> Result<()> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    cancel_upload_impl(state, session).await
}

pub async fn cancel_upload_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
) -> Result<()> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    cancel_upload_impl(state, session).await
}

/// 清理过期的上传 session（后台任务调用）
pub async fn cleanup_expired(state: &AppState) -> Result<u32> {
    let expired = upload_session_repo::find_expired(&state.db).await?;
    let count = expired.len() as u32;
    for session in expired {
        if let Some(ref temp_key) = session.s3_temp_key
            && let Some(policy) = state.policy_snapshot.get_policy(session.policy_id)
            && let Ok(driver) = state.driver_registry.get_driver(&policy)
        {
            if let Some(ref multipart_id) = session.s3_multipart_id {
                if let Some(multipart) = driver.as_multipart() {
                    if let Err(error) = multipart
                        .abort_multipart_upload(temp_key, multipart_id)
                        .await
                    {
                        tracing::warn!("failed to abort expired S3 multipart upload: {error}");
                    }
                }
                if let Err(error) = driver.delete(temp_key).await {
                    tracing::warn!("failed to delete expired S3 temp object after abort: {error}");
                }
            } else if let Err(error) = driver.delete(temp_key).await {
                tracing::warn!("failed to delete S3 temp object: {error}");
            }
        }
        cleanup_upload_temp_dir(state, &session.id).await;
        if let Err(error) = upload_session_repo::delete(&state.db, &session.id).await {
            tracing::warn!(
                "failed to delete expired upload session {}: {error}",
                session.id
            );
        }
    }
    if count > 0 {
        tracing::info!("cleaned up {count} expired upload sessions");
    }
    Ok(count)
}
