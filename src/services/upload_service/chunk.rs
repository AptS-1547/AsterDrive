//! 分片上传阶段。
//!
//! 这里处理两类“已经进入分片模式”的 session：
//! - 服务端本地暂存 chunk 文件
//! - 服务端 relay 到 S3 multipart，并把 ETag 记入 upload_session_parts

use chrono::Utc;

use crate::db::repository::{upload_session_part_repo, upload_session_repo};
use crate::entities::upload_session;
use crate::errors::{
    AsterError, MapAsterErr, Result, chunk_upload_error_with_subcode, validation_error_with_subcode,
};
use crate::runtime::PrimaryAppState;
use crate::services::upload_service::responses::ChunkUploadResponse;
use crate::services::upload_service::scope::{load_upload_session, personal_scope, team_scope};
use crate::services::upload_service::shared::{
    expected_chunk_size_for_upload, upload_session_chunk_unavailable_error,
};
use crate::types::UploadSessionStatus;
use crate::utils::numbers::usize_to_i64;
use crate::utils::paths;

async fn increment_session_received_count<C: sea_orm::ConnectionTrait>(
    db: &C,
    upload_id: &str,
) -> Result<()> {
    if upload_session_repo::increment_received_count_if_uploading(db, upload_id).await? {
        return Ok(());
    }

    // 计数自增失败不代表数据库坏了，更常见的是 session 状态已经不再允许继续上传。
    // 回读最新 session 后返回更准确的业务错误，避免客户端只看到模糊的 DB 失败。
    match upload_session_repo::find_by_id(db, upload_id).await {
        Ok(session) => Err(upload_session_chunk_unavailable_error(&session)),
        Err(error) => Err(error),
    }
}

async fn upload_chunk_impl(
    state: &PrimaryAppState,
    session: upload_session::Model,
    chunk_number: i32,
    data: &[u8],
) -> Result<ChunkUploadResponse> {
    let db = &state.db;
    let upload_id = session.id.as_str();
    tracing::debug!(
        upload_id,
        chunk_number,
        chunk_size = data.len(),
        status = ?session.status,
        total_chunks = session.total_chunks,
        "handling upload chunk"
    );
    if session.status != UploadSessionStatus::Uploading {
        return Err(upload_session_chunk_unavailable_error(&session));
    }
    if session.expires_at < Utc::now() {
        return Err(AsterError::upload_session_expired("session expired"));
    }
    if chunk_number < 0 || chunk_number >= session.total_chunks {
        return Err(validation_error_with_subcode(
            "upload.chunk_number_out_of_range",
            format!(
                "chunk_number {} out of range [0, {})",
                chunk_number, session.total_chunks
            ),
        ));
    }

    let expected_size = expected_chunk_size_for_upload(&session, chunk_number)?;
    let data_len = usize_to_i64(data.len(), "chunk data length")?;
    if data_len != expected_size {
        return Err(chunk_upload_error_with_subcode(
            "upload.chunk_size_mismatch",
            format!("chunk {chunk_number} size mismatch: expected {expected_size}, got {data_len}"),
        ));
    }

    if let (Some(temp_key), Some(multipart_id)) = (
        session.s3_temp_key.as_deref(),
        session.s3_multipart_id.as_deref(),
    ) {
        let s3_part_number = chunk_number + 1;

        // relay multipart 下，先 claim part 再上传到 S3。
        // 否则并发重试会把同一个 part 号重复上传，最后谁的 ETag 留库就会变得不确定。
        if !upload_session_part_repo::try_claim_part(db, upload_id, s3_part_number).await? {
            let updated = upload_session_repo::find_by_id(db, upload_id).await?;
            tracing::debug!(
                upload_id,
                chunk_number,
                part_number = s3_part_number,
                received_count = updated.received_count,
                total_chunks = updated.total_chunks,
                "skipping already claimed relay multipart part"
            );
            return Ok(ChunkUploadResponse {
                received_count: updated.received_count,
                total_chunks: updated.total_chunks,
            });
        }

        let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
        let multipart = state.driver_registry.get_multipart_driver(&policy)?;
        let etag = match multipart
            .upload_multipart_part(temp_key, multipart_id, s3_part_number, data)
            .await
        {
            Ok(etag) => etag,
            Err(err) => {
                if let Err(cleanup_err) = upload_session_part_repo::delete_by_upload_and_part(
                    db,
                    upload_id,
                    s3_part_number,
                )
                .await
                {
                    tracing::warn!(
                        upload_id,
                        part_number = s3_part_number,
                        "failed to release relay multipart part claim after upload error: {cleanup_err}"
                    );
                }
                return Err(err);
            }
        };

        let txn = crate::db::transaction::begin(db).await?;
        let finalize_result = async {
            // S3 上传成功以后，必须把 part 元数据和 received_count 放在同一事务里提交；
            // 否则 complete 阶段会看到“不完整的 part 清单”。
            upload_session_part_repo::upsert_part(&txn, upload_id, s3_part_number, &etag, data_len)
                .await?;
            increment_session_received_count(&txn, upload_id).await?;
            crate::db::transaction::commit(txn).await?;
            Ok::<(), AsterError>(())
        }
        .await;

        if let Err(err) = finalize_result {
            if let Err(cleanup_err) =
                upload_session_part_repo::delete_by_upload_and_part(db, upload_id, s3_part_number)
                    .await
            {
                tracing::warn!(
                    upload_id,
                    part_number = s3_part_number,
                    "failed to release relay multipart part claim after DB finalize error: {cleanup_err}"
                );
            }
            return Err(err);
        }

        let updated = upload_session_repo::find_by_id(db, upload_id).await?;
        tracing::debug!(
            upload_id,
            chunk_number,
            part_number = s3_part_number,
            received_count = updated.received_count,
            total_chunks = updated.total_chunks,
            "stored relay multipart chunk"
        );
        return Ok(ChunkUploadResponse {
            received_count: updated.received_count,
            total_chunks: updated.total_chunks,
        });
    }

    let chunk_path = paths::upload_chunk_path(
        &state.config.server.upload_temp_dir,
        upload_id,
        chunk_number,
    );

    use tokio::fs::OpenOptions;
    use tokio::io::AsyncWriteExt;
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&chunk_path)
        .await
    {
        Ok(mut file) => {
            file.write_all(data)
                .await
                .map_aster_err_ctx("write chunk", |message| {
                    chunk_upload_error_with_subcode("upload.chunk_persist_failed", message)
                })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let updated = upload_session_repo::find_by_id(db, upload_id).await?;
            tracing::debug!(
                upload_id,
                chunk_number,
                received_count = updated.received_count,
                total_chunks = updated.total_chunks,
                "skipping already uploaded chunk"
            );
            return Ok(ChunkUploadResponse {
                received_count: updated.received_count,
                total_chunks: updated.total_chunks,
            });
        }
        Err(error) => {
            return Err(chunk_upload_error_with_subcode(
                "upload.chunk_persist_failed",
                format!("create chunk file: {error}"),
            ));
        }
    }

    // 本地 chunk 模式的幂等语义靠 `create_new(true)` 保证：同一块重复上传不会覆盖旧文件，
    // 而是直接回读 session 进度返回给客户端。
    increment_session_received_count(db, upload_id).await?;

    let updated = upload_session_repo::find_by_id(db, upload_id).await?;
    tracing::debug!(
        upload_id,
        chunk_number,
        received_count = updated.received_count,
        total_chunks = updated.total_chunks,
        "stored upload chunk"
    );
    Ok(ChunkUploadResponse {
        received_count: updated.received_count,
        total_chunks: updated.total_chunks,
    })
}

/// 上传单个分片
pub async fn upload_chunk(
    state: &PrimaryAppState,
    upload_id: &str,
    chunk_number: i32,
    user_id: i64,
    data: &[u8],
) -> Result<ChunkUploadResponse> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    upload_chunk_impl(state, session, chunk_number, data).await
}

pub async fn upload_chunk_for_team(
    state: &PrimaryAppState,
    team_id: i64,
    upload_id: &str,
    chunk_number: i32,
    user_id: i64,
    data: &[u8],
) -> Result<ChunkUploadResponse> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    upload_chunk_impl(state, session, chunk_number, data).await
}
