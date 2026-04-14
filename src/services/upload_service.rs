use chrono::Utc;
use sea_orm::{Set, TransactionTrait};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::constants::HOUR_SECS;
use crate::db::repository::{file_repo, upload_session_part_repo, upload_session_repo};
use crate::entities::{file, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    workspace_models::FileInfo,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::storage::driver::StorageDriver;
use crate::types::{
    DriverType, S3UploadStrategy, UploadMode, UploadSessionStatus,
    effective_s3_multipart_chunk_size, parse_storage_policy_options,
};
use crate::utils::{id, numbers, paths};

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct InitUploadResponse {
    pub mode: UploadMode,
    pub upload_id: Option<String>,
    pub chunk_size: Option<i64>,
    pub total_chunks: Option<i32>,
    /// S3 presigned PUT URL（仅 presigned 模式）
    pub presigned_url: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ChunkUploadResponse {
    pub received_count: i32,
    pub total_chunks: i32,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UploadProgressResponse {
    pub upload_id: String,
    pub status: UploadSessionStatus,
    pub received_count: i32,
    pub chunks_on_disk: Vec<i32>,
    pub chunk_size: i64,
    pub total_chunks: i32,
    pub filename: String,
}

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
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::upload_session_not_found(format!(
            "session {upload_id}"
        )));
    }

    Ok(())
}

fn expected_chunk_size_for_upload(
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
async fn generate_upload_id<C: sea_orm::ConnectionTrait>(db: &C) -> Result<String> {
    for _ in 0..5 {
        let candidate = id::new_uuid();
        match upload_session_repo::find_by_id(db, &candidate).await {
            Err(e) if e.code() == "E054" => return Ok(candidate), // NotFound → 可用
            Err(e) => return Err(e),                              // 真实 DB 错误向上传播
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

fn ensure_personal_upload_session_scope(session: &upload_session::Model) -> Result<()> {
    if session.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "upload session belongs to a team workspace",
        ));
    }
    Ok(())
}

fn ensure_team_upload_session_scope(session: &upload_session::Model, team_id: i64) -> Result<()> {
    if session.team_id != Some(team_id) {
        return Err(AsterError::auth_forbidden(
            "upload session is outside team workspace",
        ));
    }
    Ok(())
}

async fn load_upload_session(
    state: &AppState,
    scope: WorkspaceStorageScope,
    upload_id: &str,
) -> Result<upload_session::Model> {
    let session = upload_session_repo::find_by_id(&state.db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, scope.actor_user_id(), "upload session")?;
    if let Some(team_id) = scope.team_id() {
        workspace_storage_service::require_team_access(state, team_id, scope.actor_user_id())
            .await?;
        ensure_team_upload_session_scope(&session, team_id)?;
    } else {
        ensure_personal_upload_session_scope(&session)?;
    }
    Ok(session)
}

async fn init_upload_for_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    let db = &state.db;
    let user_id = scope.actor_user_id();
    let team_id = scope.team_id();

    tracing::debug!(
        scope = ?scope,
        folder_id,
        filename = %filename,
        total_size,
        relative_path = relative_path.unwrap_or(""),
        "initializing upload session"
    );

    let (resolved_folder_id, resolved_filename) = match relative_path {
        Some(path) => {
            let parsed = workspace_storage_service::parse_relative_upload_path(
                state, scope, folder_id, path,
            )
            .await?;
            let resolved_folder_id =
                workspace_storage_service::ensure_upload_parent_path(state, scope, &parsed).await?;
            (resolved_folder_id, parsed.filename)
        }
        None => {
            crate::utils::validate_name(filename)?;
            if let Some(folder_id) = folder_id {
                workspace_storage_service::verify_folder_access(state, scope, folder_id).await?;
            }
            (folder_id, filename.to_string())
        }
    };

    tracing::debug!(
        scope = ?scope,
        folder_id = resolved_folder_id,
        filename = %resolved_filename,
        "resolved upload session target"
    );

    let policy = workspace_storage_service::resolve_policy_for_size(
        state,
        scope,
        resolved_folder_id,
        total_size,
    )
    .await?;

    tracing::debug!(
        scope = ?scope,
        policy_id = policy.id,
        driver_type = ?policy.driver_type,
        chunk_size = policy.chunk_size,
        total_size,
        "resolved upload storage policy"
    );

    if policy.max_file_size > 0 && total_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            total_size, policy.max_file_size
        )));
    }

    workspace_storage_service::check_quota(db, scope, total_size).await?;

    if policy.driver_type == DriverType::S3 {
        let opts = parse_storage_policy_options(&policy.options);
        let strategy = opts.effective_s3_upload_strategy();
        if strategy == S3UploadStrategy::Presigned {
            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = generate_upload_id(db).await?;
            let temp_key = format!("files/{upload_id}");
            let chunk_size = effective_s3_multipart_chunk_size(policy.chunk_size);

            if policy.chunk_size == 0 || total_size <= chunk_size {
                let presigned_url = driver
                    .presigned_put_url(&temp_key, std::time::Duration::from_secs(HOUR_SECS))
                    .await?
                    .ok_or_else(|| {
                        AsterError::storage_driver_error("presigned PUT not supported by driver")
                    })?;

                let now = Utc::now();
                let expires_at = now + chrono::Duration::hours(1);

                let session = upload_session::ActiveModel {
                    id: Set(upload_id.clone()),
                    user_id: Set(user_id),
                    team_id: Set(team_id),
                    filename: Set(resolved_filename.clone()),
                    total_size: Set(total_size),
                    chunk_size: Set(0),
                    total_chunks: Set(0),
                    received_count: Set(0),
                    folder_id: Set(resolved_folder_id),
                    policy_id: Set(policy.id),
                    status: Set(UploadSessionStatus::Presigned),
                    s3_temp_key: Set(Some(temp_key)),
                    s3_multipart_id: Set(None),
                    file_id: Set(None),
                    created_at: Set(now),
                    expires_at: Set(expires_at),
                    updated_at: Set(now),
                };
                upload_session_repo::create(db, session).await?;

                tracing::debug!(
                    scope = ?scope,
                    upload_id = %upload_id,
                    policy_id = policy.id,
                    mode = ?UploadMode::Presigned,
                    folder_id = resolved_folder_id,
                    "initialized presigned upload session"
                );

                return Ok(InitUploadResponse {
                    mode: UploadMode::Presigned,
                    upload_id: Some(upload_id),
                    chunk_size: None,
                    total_chunks: None,
                    presigned_url: Some(presigned_url),
                });
            }

            // 大文件 → S3 multipart presigned 直传
            let s3_upload_id = driver.create_multipart_upload(&temp_key).await?;
            let total_chunks =
                numbers::calc_total_chunks(total_size, chunk_size, "presigned multipart upload")?;

            let now = Utc::now();
            let expires_at = now + chrono::Duration::hours(24);

            let session = upload_session::ActiveModel {
                id: Set(upload_id.clone()),
                user_id: Set(user_id),
                team_id: Set(team_id),
                filename: Set(resolved_filename.clone()),
                total_size: Set(total_size),
                chunk_size: Set(chunk_size),
                total_chunks: Set(total_chunks),
                received_count: Set(0),
                folder_id: Set(resolved_folder_id),
                policy_id: Set(policy.id),
                status: Set(UploadSessionStatus::Presigned),
                s3_temp_key: Set(Some(temp_key)),
                s3_multipart_id: Set(Some(s3_upload_id)),
                file_id: Set(None),
                created_at: Set(now),
                expires_at: Set(expires_at),
                updated_at: Set(now),
            };
            upload_session_repo::create(db, session).await?;

            tracing::debug!(
                scope = ?scope,
                upload_id = %upload_id,
                policy_id = policy.id,
                mode = ?UploadMode::PresignedMultipart,
                chunk_size,
                total_chunks,
                folder_id = resolved_folder_id,
                "initialized presigned multipart upload session"
            );

            return Ok(InitUploadResponse {
                mode: UploadMode::PresignedMultipart,
                upload_id: Some(upload_id),
                chunk_size: Some(chunk_size),
                total_chunks: Some(total_chunks),
                presigned_url: None,
            });
        }

        if strategy == S3UploadStrategy::RelayStream {
            let chunk_size = effective_s3_multipart_chunk_size(policy.chunk_size);
            if policy.chunk_size == 0 || total_size <= chunk_size {
                tracing::debug!(
                    scope = ?scope,
                    policy_id = policy.id,
                    mode = ?UploadMode::Direct,
                    folder_id = resolved_folder_id,
                    "selected direct relay upload mode"
                );
                return Ok(InitUploadResponse {
                    mode: UploadMode::Direct,
                    upload_id: None,
                    chunk_size: None,
                    total_chunks: None,
                    presigned_url: None,
                });
            }

            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = generate_upload_id(db).await?;
            let temp_key = format!("files/{upload_id}");
            let s3_upload_id = driver.create_multipart_upload(&temp_key).await?;
            let total_chunks =
                numbers::calc_total_chunks(total_size, chunk_size, "relay multipart upload")?;
            let now = Utc::now();
            let expires_at = now + chrono::Duration::hours(24);

            let session = upload_session::ActiveModel {
                id: Set(upload_id.clone()),
                user_id: Set(user_id),
                team_id: Set(team_id),
                filename: Set(resolved_filename.clone()),
                total_size: Set(total_size),
                chunk_size: Set(chunk_size),
                total_chunks: Set(total_chunks),
                received_count: Set(0),
                folder_id: Set(resolved_folder_id),
                policy_id: Set(policy.id),
                status: Set(UploadSessionStatus::Uploading),
                s3_temp_key: Set(Some(temp_key)),
                s3_multipart_id: Set(Some(s3_upload_id)),
                file_id: Set(None),
                created_at: Set(now),
                expires_at: Set(expires_at),
                updated_at: Set(now),
            };
            upload_session_repo::create(db, session).await?;

            tracing::debug!(
                scope = ?scope,
                upload_id = %upload_id,
                policy_id = policy.id,
                mode = ?UploadMode::Chunked,
                chunk_size,
                total_chunks,
                folder_id = resolved_folder_id,
                "initialized relay multipart upload session"
            );

            return Ok(InitUploadResponse {
                mode: UploadMode::Chunked,
                upload_id: Some(upload_id),
                chunk_size: Some(chunk_size),
                total_chunks: Some(total_chunks),
                presigned_url: None,
            });
        }
    }

    if policy.chunk_size == 0 || total_size <= policy.chunk_size {
        tracing::debug!(
            scope = ?scope,
            policy_id = policy.id,
            mode = ?UploadMode::Direct,
            folder_id = resolved_folder_id,
            "selected direct upload mode"
        );
        return Ok(InitUploadResponse {
            mode: UploadMode::Direct,
            upload_id: None,
            chunk_size: None,
            total_chunks: None,
            presigned_url: None,
        });
    }

    let chunk_size = policy.chunk_size;
    let total_chunks = numbers::calc_total_chunks(total_size, chunk_size, "chunked upload")?;
    let upload_id = generate_upload_id(db).await?;
    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(24);

    let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, &upload_id);
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::chunk_upload_failed)?;

    let session = upload_session::ActiveModel {
        id: Set(upload_id.clone()),
        user_id: Set(user_id),
        team_id: Set(team_id),
        filename: Set(resolved_filename.clone()),
        total_size: Set(total_size),
        chunk_size: Set(chunk_size),
        total_chunks: Set(total_chunks),
        received_count: Set(0),
        folder_id: Set(resolved_folder_id),
        policy_id: Set(policy.id),
        status: Set(UploadSessionStatus::Uploading),
        s3_temp_key: Set(None),
        s3_multipart_id: Set(None),
        file_id: Set(None),
        created_at: Set(now),
        expires_at: Set(expires_at),
        updated_at: Set(now),
    };
    upload_session_repo::create(db, session).await?;

    tracing::debug!(
        scope = ?scope,
        upload_id = %upload_id,
        policy_id = policy.id,
        mode = ?UploadMode::Chunked,
        chunk_size,
        total_chunks,
        folder_id = resolved_folder_id,
        "initialized chunked upload session"
    );

    Ok(InitUploadResponse {
        mode: UploadMode::Chunked,
        upload_id: Some(upload_id),
        chunk_size: Some(chunk_size),
        total_chunks: Some(total_chunks),
        presigned_url: None,
    })
}

/// 上传协商：服务端根据存储策略决定上传模式
pub async fn init_upload(
    state: &AppState,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    init_upload_for_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
}

pub async fn init_upload_for_team(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    init_upload_for_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
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
        return Err(AsterError::chunk_upload_failed(format!(
            "session status is '{:?}', expected 'uploading'",
            session.status
        )));
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

    // 用 create_new (O_EXCL) 原子创建文件，已存在则幂等返回
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
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // 幂等：分片已上传过，直接返回当前进度
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
        Err(e) => {
            return Err(AsterError::chunk_upload_failed(format!(
                "create chunk file: {e}"
            )));
        }
    }

    // 原子 +1（sea-query Expr 避免 read-modify-write race condition）
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
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Personal { user_id },
        upload_id,
    )
    .await?;
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
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        upload_id,
    )
    .await?;
    upload_chunk_impl(state, session, chunk_number, data).await
}

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

    // ── 幂等性处理：如果已完成，返回对应文件 ──
    if session.status == UploadSessionStatus::Completed {
        return find_file_by_session(db, &session).await;
    }

    // ── 如果正在处理中，返回友好提示（前端轮询重试） ──
    if session.status == UploadSessionStatus::Assembling {
        return Err(AsterError::upload_assembling(
            "upload is being processed, please wait and retry in a few seconds",
        ));
    }

    // ── 如果 assembly 之前失败过，明确告知（不能再 complete） ──
    if session.status == UploadSessionStatus::Failed {
        return Err(AsterError::upload_assembly_failed(
            "upload assembly failed previously; please start a new upload",
        ));
    }

    // Presigned 模式走独立流程
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

    // ── 原子状态转换 uploading → assembling（防止并发 complete 双重触发） ──
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

    // ── [事务外] 流式拼接分片；local 未开启 dedup 时跳过 sha256 ──
    // 任何失败都将 session 标记为 Failed，避免前端无限轮询 Assembling
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
        let txn = state.db.begin().await.map_err(AsterError::from)?;

        let blob = if let Some(hasher) = hasher {
            let file_hash = crate::utils::hash::sha256_digest_to_hex(&hasher.finalize());
            let storage_path = crate::utils::storage_path_from_hash(&file_hash);
            let blob =
                file_repo::find_or_create_blob(&txn, &file_hash, size, policy.id, &storage_path)
                    .await?;
            if blob.inserted {
                // 零拷贝：LocalDriver rename，S3 流式上传，不读进内存
                driver.put_file(&storage_path, &assembled_path).await?;
            } else {
                crate::utils::cleanup_temp_file(&assembled_path).await;
            }
            blob.model
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
        Ok(created)
    }
    .await;

    match result {
        Ok(created) => {
            // ── [事务外] 清理临时文件 ──
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
        Err(e) => {
            // 将 session 标记为 Failed，防止前端轮询 Assembling 永不退出
            mark_session_failed(db, upload_id).await;
            Err(e)
        }
    }
}

pub async fn complete_upload(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
    parts: Option<Vec<(i32, String)>>,
) -> Result<FileInfo> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Personal { user_id },
        upload_id,
    )
    .await?;
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
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        upload_id,
    )
    .await?;
    complete_upload_impl(state, session, parts)
        .await
        .map(FileInfo::from)
}

fn upload_session_status_label(status: UploadSessionStatus) -> &'static str {
    match status {
        UploadSessionStatus::Uploading => "uploading",
        UploadSessionStatus::Assembling => "assembling",
        UploadSessionStatus::Completed => "completed",
        UploadSessionStatus::Failed => "failed",
        UploadSessionStatus::Presigned => "presigned",
    }
}

async fn transition_upload_session_to_assembling<C: sea_orm::ConnectionTrait>(
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
        if let Err(e) = driver.delete(temp_key).await {
            tracing::warn!("failed to delete S3 temp object: {e}");
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
        Err(e) => {
            mark_session_failed(db, &upload_id).await;
            Err(e)
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
        Ok(f) => {
            tracing::debug!(
                upload_id = %upload_id,
                file_id = f.id,
                blob_id = f.blob_id,
                size = f.size,
                "completed presigned upload session"
            );
            Ok(f)
        }
        Err(e) => {
            mark_session_failed(db, &upload_id).await;
            Err(e)
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
        numbers::i32_to_usize(session.total_chunks, "upload session total_chunks")?;
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

/// 将 session 标记为 Failed（best-effort，失败只记录日志）
async fn mark_session_failed<C: sea_orm::ConnectionTrait>(db: &C, upload_id: &str) {
    if let Ok(s) = upload_session_repo::find_by_id(db, upload_id).await {
        let mut active: upload_session::ActiveModel = s.into();
        active.status = Set(UploadSessionStatus::Failed);
        active.updated_at = Set(Utc::now());
        if let Err(e) = upload_session_repo::update(db, active).await {
            tracing::warn!("failed to mark session {upload_id} as failed: {e}");
        }
    }
}

/// 根据 session 查找已完成的文件（幂等重试用）
async fn find_file_by_session<C: sea_orm::ConnectionTrait>(
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

    // 清理 S3 临时对象 / multipart upload
    if let Some(ref temp_key) = session.s3_temp_key {
        let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
        if let Ok(driver) = state.driver_registry.get_driver(&policy) {
            if let Some(ref multipart_id) = session.s3_multipart_id {
                if let Err(e) = driver.abort_multipart_upload(temp_key, multipart_id).await {
                    tracing::warn!("failed to abort S3 multipart upload: {e}");
                }
            } else if let Err(e) = driver.delete(temp_key).await {
                tracing::warn!("failed to delete S3 temp object: {e}");
            }
        }
    }

    let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, upload_id);
    crate::utils::cleanup_temp_dir(&temp_dir).await;
    upload_session_repo::delete(&state.db, upload_id).await?;
    tracing::debug!(upload_id, "canceled upload session");
    Ok(())
}

pub async fn cancel_upload(state: &AppState, upload_id: &str, user_id: i64) -> Result<()> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Personal { user_id },
        upload_id,
    )
    .await?;
    cancel_upload_impl(state, session).await
}

pub async fn cancel_upload_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
) -> Result<()> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        upload_id,
    )
    .await?;
    cancel_upload_impl(state, session).await
}

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
    // S3 relay multipart 在整个生命周期都以 upload_session_parts 为准；
    // S3 presigned multipart 仅在 Presigned 阶段查询远端已上传 parts；
    // 其他上传模式仍按本地临时分片扫描。
    let chunks_on_disk = if let Some(multipart_id) = session.s3_multipart_id.as_deref() {
        let policy = state.policy_snapshot.get_policy_or_err(session.policy_id)?;
        let strategy = if policy.driver_type == DriverType::S3 {
            Some(parse_storage_policy_options(&policy.options).effective_s3_upload_strategy())
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
                    let driver = state.driver_registry.get_driver(&policy)?;
                    driver.list_uploaded_parts(temp_key, multipart_id).await?
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
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Personal { user_id },
        upload_id,
    )
    .await?;
    get_progress_impl(state, session).await
}

pub async fn get_progress_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
) -> Result<UploadProgressResponse> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        upload_id,
    )
    .await?;
    get_progress_impl(state, session).await
}

/// 为 S3 multipart presigned 上传批量生成 per-part presigned PUT URL
async fn presign_parts_impl(
    state: &AppState,
    session: upload_session::Model,
    part_numbers: Vec<i32>,
) -> Result<std::collections::HashMap<i32, String>> {
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
    let driver = state.driver_registry.get_driver(&policy)?;

    let expires = std::time::Duration::from_secs(HOUR_SECS);
    let mut urls = std::collections::HashMap::new();
    for part_num in part_numbers {
        let url = driver
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
) -> Result<std::collections::HashMap<i32, String>> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Personal { user_id },
        upload_id,
    )
    .await?;
    presign_parts_impl(state, session, part_numbers).await
}

pub async fn presign_parts_for_team(
    state: &AppState,
    team_id: i64,
    upload_id: &str,
    user_id: i64,
    part_numbers: Vec<i32>,
) -> Result<std::collections::HashMap<i32, String>> {
    let session = load_upload_session(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        upload_id,
    )
    .await?;
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

/// 清理过期的上传 session（后台任务调用）
pub async fn cleanup_expired(state: &AppState) -> Result<u32> {
    let expired = upload_session_repo::find_expired(&state.db).await?;
    let count = expired.len() as u32;
    for session in expired {
        // 清理 S3 临时对象 / multipart upload
        if let Some(ref temp_key) = session.s3_temp_key
            && let Some(policy) = state.policy_snapshot.get_policy(session.policy_id)
            && let Ok(driver) = state.driver_registry.get_driver(&policy)
        {
            if let Some(ref multipart_id) = session.s3_multipart_id {
                if let Err(e) = driver.abort_multipart_upload(temp_key, multipart_id).await {
                    tracing::warn!("failed to abort expired S3 multipart upload: {e}");
                }
            } else if let Err(e) = driver.delete(temp_key).await {
                tracing::warn!("failed to delete S3 temp object: {e}");
            }
        }
        let temp_dir = paths::upload_temp_dir(&state.config.server.upload_temp_dir, &session.id);
        crate::utils::cleanup_temp_dir(&temp_dir).await;
        if let Err(e) = upload_session_repo::delete(&state.db, &session.id).await {
            tracing::warn!(
                "failed to delete expired upload session {}: {e}",
                session.id
            );
        }
    }
    if count > 0 {
        tracing::info!("cleaned up {count} expired upload sessions");
    }
    Ok(count)
}
