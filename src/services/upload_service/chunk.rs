use chrono::Utc;
use sea_orm::TransactionTrait;

use crate::db::repository::{upload_session_part_repo, upload_session_repo};
use crate::entities::upload_session;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::upload_service::responses::ChunkUploadResponse;
use crate::services::upload_service::scope::{load_upload_session, personal_scope, team_scope};
use crate::services::upload_service::shared::{
    expected_chunk_size_for_upload, upload_session_chunk_unavailable_error,
};
use crate::types::UploadSessionStatus;
use crate::utils::paths;

async fn increment_session_received_count<C: sea_orm::ConnectionTrait>(
    db: &C,
    upload_id: &str,
) -> Result<()> {
    use crate::entities::upload_session::{Column, Entity as UploadSession};
    use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, QueryFilter, sea_query::Expr};

    let result = UploadSession::update_many()
        .col_expr(
            Column::ReceivedCount,
            Expr::col(Column::ReceivedCount).add(1),
        )
        .col_expr(Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(Column::Id.eq(upload_id))
        .filter(Column::Status.eq(UploadSessionStatus::Uploading))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 1 {
        return Ok(());
    }

    match upload_session_repo::find_by_id(db, upload_id).await {
        Ok(session) => Err(upload_session_chunk_unavailable_error(&session)),
        Err(error) => Err(error),
    }
}

async fn upload_chunk_impl(
    state: &AppState,
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
        return Err(AsterError::validation_error(format!(
            "chunk_number {} out of range [0, {})",
            chunk_number, session.total_chunks
        )));
    }

    let expected_size = expected_chunk_size_for_upload(&session, chunk_number)?;
    if data.len() as i64 != expected_size {
        return Err(AsterError::chunk_upload_failed(format!(
            "chunk {chunk_number} size mismatch: expected {expected_size}, got {}",
            data.len()
        )));
    }

    if let (Some(temp_key), Some(multipart_id)) = (
        session.s3_temp_key.as_deref(),
        session.s3_multipart_id.as_deref(),
    ) {
        let s3_part_number = chunk_number + 1;

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
        let driver = state.driver_registry.get_driver(&policy)?;
        let etag = match driver
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

        let txn = db.begin().await.map_err(AsterError::from)?;
        let finalize_result = async {
            upload_session_part_repo::upsert_part(
                &txn,
                upload_id,
                s3_part_number,
                &etag,
                data.len() as i64,
            )
            .await?;
            increment_session_received_count(&txn, upload_id).await?;
            txn.commit().await.map_err(AsterError::from)?;
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
                .map_aster_err_ctx("write chunk", AsterError::chunk_upload_failed)?;
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
            return Err(AsterError::chunk_upload_failed(format!(
                "create chunk file: {error}"
            )));
        }
    }

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
    state: &AppState,
    upload_id: &str,
    chunk_number: i32,
    user_id: i64,
    data: &[u8],
) -> Result<ChunkUploadResponse> {
    let session = load_upload_session(state, personal_scope(user_id), upload_id).await?;
    upload_chunk_impl(state, session, chunk_number, data).await
}

pub async fn upload_chunk_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    chunk_number: i32,
    user_id: i64,
    data: &[u8],
) -> Result<ChunkUploadResponse> {
    let session = load_upload_session(state, team_scope(team_id, user_id), upload_id).await?;
    upload_chunk_impl(state, session, chunk_number, data).await
}
