//! 上传初始化阶段。
//!
//! 这里不真正写入文件内容，只负责：
//! - 解析目标路径和目录自动创建
//! - 解析存储策略与大小限制
//! - 协商最终上传模式
//! - 在需要 session 的模式下预先写入 upload_sessions

use chrono::{Duration, Utc};
use sea_orm::Set;

use crate::api::constants::HOUR_SECS;
use crate::db::repository::upload_session_repo;
use crate::entities::upload_session;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::upload_service::responses::InitUploadResponse;
use crate::services::upload_service::scope::{personal_scope, team_scope};
use crate::services::upload_service::shared::generate_upload_id;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};
use crate::types::{
    DriverType, S3UploadStrategy, UploadMode, UploadSessionStatus,
    effective_s3_multipart_chunk_size, parse_storage_policy_options,
};
use crate::utils::{numbers, paths};

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
            // 目录上传会把 `relative_path` 拆成“父目录链 + 最终文件名”。
            // 这里就把目录路径补齐，后续模式选择和 session 记录都只看解析后的最终目标。
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

    // upload 模式协商建立在“最终会写到哪条策略”之上，而不是客户端自己传 mode。
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
        let opts = parse_storage_policy_options(policy.options.as_ref());
        let strategy = opts.effective_s3_upload_strategy();
        if strategy == S3UploadStrategy::Presigned {
            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = generate_upload_id(db).await?;
            let temp_key = format!("files/{upload_id}");
            let chunk_size = effective_s3_multipart_chunk_size(policy.chunk_size);

            // 小文件 presigned：客户端直接 PUT 到最终 temp object，不经过服务端 relay，
            // 也不需要 chunk bookkeeping。
            if policy.chunk_size == 0 || total_size <= chunk_size {
                let presigned_url = driver
                    .presigned_put_url(&temp_key, std::time::Duration::from_secs(HOUR_SECS))
                    .await?
                    .ok_or_else(|| {
                        AsterError::storage_driver_error("presigned PUT not supported by driver")
                    })?;

                let now = Utc::now();
                let expires_at = now + Duration::hours(1);

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

            // 大文件 presigned multipart：服务端仍然不接管数据流，但必须保留 session，
            // 用来记录 multipart upload_id、分片总数以及后续 complete 阶段的收口点。
            let s3_upload_id = driver.create_multipart_upload(&temp_key).await?;
            let total_chunks =
                numbers::calc_total_chunks(total_size, chunk_size, "presigned multipart upload")?;

            let now = Utc::now();
            let expires_at = now + Duration::hours(24);

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
            // relay_stream + 小文件：直接走普通上传接口，让服务端把字节流转发到驱动。
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

            // relay_stream + 大文件：客户端仍然分片传给服务端，服务端再逐片上传到 S3 multipart。
            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = generate_upload_id(db).await?;
            let temp_key = format!("files/{upload_id}");
            let s3_upload_id = driver.create_multipart_upload(&temp_key).await?;
            let total_chunks =
                numbers::calc_total_chunks(total_size, chunk_size, "relay multipart upload")?;
            let now = Utc::now();
            let expires_at = now + Duration::hours(24);

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

    // 非 S3 或未启用 multipart 时，小文件直接走 direct upload，不需要 upload session。
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

    // 本地 / 其他非 direct 场景：服务端维护分片目录与 upload session，
    // complete 阶段会把这些 chunk 组装成最终文件。
    let chunk_size = policy.chunk_size;
    let total_chunks = numbers::calc_total_chunks(total_size, chunk_size, "chunked upload")?;
    let upload_id = generate_upload_id(db).await?;
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);

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
        personal_scope(user_id),
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
}

/// 团队空间上传协商：规则和个人空间一致，但路径归属与配额都落在团队 scope。
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
        team_scope(team_id, user_id),
        filename,
        total_size,
        folder_id,
        relative_path,
    )
    .await
}
