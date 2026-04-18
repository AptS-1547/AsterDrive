//! 上传服务子模块：`progress`。

use std::collections::HashMap;

use crate::api::constants::HOUR_SECS;
use crate::db::repository::upload_session_part_repo;
use crate::entities::upload_session;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::upload_service::responses::UploadProgressResponse;
use crate::services::upload_service::scope::{load_upload_session, personal_scope, team_scope};
use crate::types::{
    DriverType, S3UploadStrategy, UploadSessionStatus, parse_storage_policy_options,
};
use crate::utils::paths;

/// 查询上传进度
async fn get_progress_impl(
    state: &AppState,
    session: upload_session::Model,
) -> Result<UploadProgressResponse> {
    tracing::debug!(
        upload_id = %session.id,
        status = ?session.status,
        total_chunks = session.total_chunks,
        received_count = session.received_count,
        "loading upload progress"
    );

    let chunks_on_disk = if let Some(multipart_id) = session.s3_multipart_id.as_deref() {
        let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
        let strategy = if policy.driver_type == DriverType::S3 {
            Some(
                parse_storage_policy_options(policy.options.as_ref())
                    .effective_s3_upload_strategy(),
            )
        } else {
            None
        };

        match strategy {
            Some(S3UploadStrategy::RelayStream) => {
                upload_session_part_repo::list_part_numbers(&state.db, &session.id)
                    .await?
                    .into_iter()
                    .map(|part_number| part_number - 1)
                    .collect()
            }
            Some(S3UploadStrategy::Presigned)
                if session.status == UploadSessionStatus::Presigned =>
            {
                if let Some(temp_key) = session.s3_temp_key.as_deref() {
                    state
                        .driver_registry
                        .get_multipart_driver(&policy)?
                        .list_uploaded_parts(temp_key, multipart_id)
                        .await?
                } else {
                    scan_received_chunks(state, &session.id).await
                }
            }
            _ => scan_received_chunks(state, &session.id).await,
        }
    } else {
        scan_received_chunks(state, &session.id).await
    };

    let progress = UploadProgressResponse {
        upload_id: session.id,
        status: session.status,
        received_count: session.received_count,
        chunks_on_disk,
        chunk_size: session.chunk_size,
        total_chunks: session.total_chunks,
        filename: session.filename,
    };
    tracing::debug!(
        upload_id = %progress.upload_id,
        status = ?progress.status,
        received_count = progress.received_count,
        total_chunks = progress.total_chunks,
        chunk_count = progress.chunks_on_disk.len(),
        "loaded upload progress"
    );
    Ok(progress)
}

pub async fn get_progress(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
) -> Result<UploadProgressResponse> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    get_progress_impl(state, session).await
}

pub async fn get_progress_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
) -> Result<UploadProgressResponse> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    get_progress_impl(state, session).await
}

/// 为 S3 multipart presigned 上传批量生成 per-part presigned PUT URL
async fn presign_parts_impl(
    state: &AppState,
    session: upload_session::Model,
    part_numbers: Vec<i32>,
) -> Result<HashMap<i32, String>> {
    tracing::debug!(
        upload_id = %session.id,
        status = ?session.status,
        requested_part_count = part_numbers.len(),
        "presigning multipart upload parts"
    );
    if session.status != UploadSessionStatus::Presigned {
        return Err(AsterError::validation_error(format!(
            "session status is '{:?}', expected 'presigned'",
            session.status
        )));
    }

    let multipart_id = session
        .s3_multipart_id
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("not a multipart upload session"))?;
    let temp_key = session
        .s3_temp_key
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("missing s3_temp_key"))?;

    let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
    let multipart = state.driver_registry.get_multipart_driver(&policy)?;

    let expires = std::time::Duration::from_secs(HOUR_SECS);
    let mut urls = HashMap::new();
    for part_num in part_numbers {
        let url = multipart
            .presigned_upload_part_url(temp_key, multipart_id, part_num, expires)
            .await?;
        urls.insert(part_num, url);
    }
    tracing::debug!(
        upload_id = %session.id,
        url_count = urls.len(),
        "presigned multipart upload parts"
    );
    Ok(urls)
}

pub async fn presign_parts(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
    part_numbers: Vec<i32>,
) -> Result<HashMap<i32, String>> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    presign_parts_impl(state, session, part_numbers).await
}

pub async fn presign_parts_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
    part_numbers: Vec<i32>,
) -> Result<HashMap<i32, String>> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    presign_parts_impl(state, session, part_numbers).await
}

/// 扫描临时目录中实际存在的 chunk 文件，返回排序后的 chunk 编号列表
async fn scan_received_chunks(state: &AppState, upload_id: &str) -> Vec<i32> {
    let dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, upload_id);
    let mut received = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
        return received;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(num_str) = name.strip_prefix("chunk_")
            && let Ok(n) = num_str.parse::<i32>()
        {
            received.push(n);
        }
    }
    received.sort();
    received
}
