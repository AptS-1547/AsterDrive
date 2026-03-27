use chrono::Utc;
use sea_orm::{Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::constants::HOUR_SECS;
use crate::db::repository::{file_repo, policy_repo, upload_session_repo, user_repo};
use crate::entities::{file, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{file_service, folder_service};
use crate::types::{DriverType, UploadMode, UploadSessionStatus};
use crate::utils::id;

#[derive(Serialize, ToSchema)]
pub struct InitUploadResponse {
    pub mode: UploadMode,
    pub upload_id: Option<String>,
    pub chunk_size: Option<i64>,
    pub total_chunks: Option<i32>,
    /// S3 presigned PUT URL（仅 presigned 模式）
    pub presigned_url: Option<String>,
}

/// 存储策略 options JSON
#[derive(Deserialize, Default)]
struct PolicyOptions {
    #[serde(default)]
    presigned_upload: bool,
}

fn parse_policy_options(options: &str) -> PolicyOptions {
    serde_json::from_str(options).unwrap_or_else(|e| {
        if !options.is_empty() && options != "{}" {
            tracing::warn!("invalid policy options JSON '{options}': {e}");
        }
        PolicyOptions::default()
    })
}

#[derive(Serialize, ToSchema)]
pub struct ChunkUploadResponse {
    pub received_count: i32,
    pub total_chunks: i32,
}

#[derive(Serialize, ToSchema)]
pub struct UploadProgressResponse {
    pub upload_id: String,
    pub status: UploadSessionStatus,
    pub received_count: i32,
    pub chunks_on_disk: Vec<i32>,
    pub total_chunks: i32,
    pub filename: String,
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

/// 上传协商：服务端根据存储策略决定上传模式
pub async fn init_upload(
    state: &AppState,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<InitUploadResponse> {
    let db = &state.db;

    let (resolved_folder_id, resolved_filename) = match relative_path {
        Some(path) => folder_service::resolve_upload_path(state, user_id, folder_id, path).await?,
        None => {
            crate::utils::validate_name(filename)?;
            if let Some(fid) = folder_id {
                folder_service::verify_folder_access(state, user_id, fid).await?;
            }
            (folder_id, filename.to_string())
        }
    };

    // 确定存储策略
    let policy = file_service::resolve_policy(state, user_id, resolved_folder_id).await?;

    // 检查文件大小限制
    if policy.max_file_size > 0 && total_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            total_size, policy.max_file_size
        )));
    }

    // 检查用户配额
    user_repo::check_quota(db, user_id, total_size).await?;

    // S3 presigned 直传：策略开启 + S3 驱动
    if policy.driver_type == DriverType::S3 {
        let opts = parse_policy_options(&policy.options);
        if opts.presigned_upload {
            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = generate_upload_id(db).await?;
            let temp_key = format!("files/{upload_id}");

            // chunk_size == 0 → 禁用分片；文件 ≤ chunk_size → 单次 presigned PUT
            if policy.chunk_size == 0 || total_size <= policy.chunk_size {
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
            let chunk_size = policy.chunk_size;
            let total_chunks = ((total_size + chunk_size - 1) / chunk_size) as i32;

            let now = Utc::now();
            let expires_at = now + chrono::Duration::hours(24);

            let session = upload_session::ActiveModel {
                id: Set(upload_id.clone()),
                user_id: Set(user_id),
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

            return Ok(InitUploadResponse {
                mode: UploadMode::PresignedMultipart,
                upload_id: Some(upload_id),
                chunk_size: Some(chunk_size),
                total_chunks: Some(total_chunks),
                presigned_url: None,
            });
        }
    }

    // 策略决策：chunk_size == 0 → 禁用分片；文件 <= chunk_size → 直传
    if policy.chunk_size == 0 || total_size <= policy.chunk_size {
        return Ok(InitUploadResponse {
            mode: UploadMode::Direct,
            upload_id: None,
            chunk_size: None,
            total_chunks: None,
            presigned_url: None,
        });
    }

    let chunk_size = policy.chunk_size;
    let total_chunks = ((total_size + chunk_size - 1) / chunk_size) as i32;
    let upload_id = generate_upload_id(db).await?;
    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(24);

    // 创建临时目录
    let temp_dir = format!("data/.uploads/{upload_id}");
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::chunk_upload_failed)?;

    let session = upload_session::ActiveModel {
        id: Set(upload_id.clone()),
        user_id: Set(user_id),
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

    Ok(InitUploadResponse {
        mode: UploadMode::Chunked,
        upload_id: Some(upload_id),
        chunk_size: Some(chunk_size),
        total_chunks: Some(total_chunks),
        presigned_url: None,
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
    let db = &state.db;
    let session = upload_session_repo::find_by_id(db, upload_id).await?;

    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;
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

    let chunk_path = format!("data/.uploads/{upload_id}/chunk_{chunk_number}");

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
    use crate::entities::upload_session::{Column, Entity as UploadSession};
    use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, QueryFilter, sea_query::Expr};
    UploadSession::update_many()
        .col_expr(
            Column::ReceivedCount,
            Expr::col(Column::ReceivedCount).add(1),
        )
        .col_expr(Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(Column::Id.eq(upload_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    let updated = upload_session_repo::find_by_id(db, upload_id).await?;
    Ok(ChunkUploadResponse {
        received_count: updated.received_count,
        total_chunks: updated.total_chunks,
    })
}

/// 完成分片上传：组装 → hash → 去重 → 写入最终存储
pub async fn complete_upload(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
    parts: Option<Vec<(i32, String)>>,
) -> Result<file::Model> {
    let db = &state.db;
    let session = upload_session_repo::find_by_id(db, upload_id).await?;

    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;

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

    // ── [事务外] 流式拼接分片 + sha256 → put_file → finalize ──
    // 任何失败都将 session 标记为 Failed，避免前端无限轮询 Assembling
    let result = async {
        use sha2::{Digest, Sha256};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        const ASSEMBLY_BUFFER_SIZE: usize = 64 * 1024;

        let assembled_path = format!("data/.uploads/{upload_id}/_assembled");
        let mut out_file = tokio::fs::File::create(&assembled_path)
            .await
            .map_aster_err_ctx("create assembled file", AsterError::upload_assembly_failed)?;
        let mut hasher = Sha256::new();
        let mut size: i64 = 0;
        let mut buffer = vec![0u8; ASSEMBLY_BUFFER_SIZE];

        for i in 0..session.total_chunks {
            let chunk_path = format!("data/.uploads/{upload_id}/chunk_{i}");
            let mut chunk_file = tokio::fs::File::open(&chunk_path)
                .await
                .map_err(|e| AsterError::upload_assembly_failed(format!("open chunk {i}: {e}")))?;

            loop {
                let n = chunk_file.read(&mut buffer).await.map_err(|e| {
                    AsterError::upload_assembly_failed(format!("read chunk {i}: {e}"))
                })?;
                if n == 0 {
                    break;
                }

                let data = &buffer[..n];
                hasher.update(data);
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

        let file_hash = crate::utils::hash::sha256_digest_to_hex(&hasher.finalize());
        let now = Utc::now();

        // ── [事务外] 获取策略 + driver + put_file ──
        let policy = policy_repo::find_by_id(db, session.policy_id).await?;
        let driver = state.driver_registry.get_driver(&policy)?;

        let storage_path = crate::utils::storage_path_from_hash(&file_hash);
        let blob_pre_exists = file_repo::find_blob_by_hash(db, &file_hash, policy.id)
            .await?
            .is_some();
        if blob_pre_exists {
            // 已有相同内容，不需要再存
            crate::utils::cleanup_temp_file(&assembled_path).await;
        } else {
            // 零拷贝：LocalDriver rename，S3 流式上传，不读进内存
            driver.put_file(&storage_path, &assembled_path).await?;
        }

        finalize_upload_session(
            state,
            &session,
            &file_hash,
            size,
            policy.id,
            &storage_path,
            now,
        )
        .await
    }
    .await;

    match result {
        Ok(created) => {
            // ── [事务外] 清理临时文件 ──
            let temp_dir = format!("data/.uploads/{upload_id}");
            crate::utils::cleanup_temp_dir(&temp_dir).await;
            Ok(created)
        }
        Err(e) => {
            // 将 session 标记为 Failed，防止前端轮询 Assembling 永不退出
            mark_session_failed(db, upload_id).await;
            Err(e)
        }
    }
}

/// 完成 presigned 上传：从 S3 临时 key 读取 → hash → 去重 → copy → 建文件记录
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

    let policy = policy_repo::find_by_id(db, session.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    // ── [事务外] S3 metadata 检查 ──
    let meta = driver.metadata(&temp_key).await.map_err(|_| {
        AsterError::upload_assembly_failed(
            "S3 temp object not found - upload may not have completed",
        )
    })?;
    let actual_size = meta.size as i64;

    if actual_size != session.total_size {
        if let Err(e) = driver.delete(&temp_key).await {
            tracing::warn!("failed to delete S3 temp object: {e}");
        }
        return Err(AsterError::upload_assembly_failed(format!(
            "size mismatch: declared {} but uploaded {}",
            session.total_size, actual_size
        )));
    }

    // ── 原子状态转换 presigned → assembling（防止并发 complete 双重触发） ──
    let upload_id = &session.id;
    let transitioned = upload_session_repo::try_transition_status(
        db,
        upload_id,
        UploadSessionStatus::Presigned,
        UploadSessionStatus::Assembling,
    )
    .await?;
    if !transitioned {
        return Err(AsterError::upload_assembly_failed(format!(
            "session status is '{:?}', expected 'presigned'",
            session.status
        )));
    }

    // ── [事务外] 直接用 temp_key 作为最终路径，不做 SHA256 也不做 copy（S3 不去重） ──
    let result = async {
        let file_hash = format!("s3-{}", upload_id); // 占位 hash，保证唯一性
        let now = Utc::now();

        let created = finalize_upload_session(
            state,
            &session,
            &file_hash,
            actual_size,
            policy.id,
            &temp_key, // 直接用 temp_key，无需 copy
            now,
        )
        .await?;

        Ok(created)
    }
    .await;

    match result {
        Ok(f) => Ok(f),
        Err(e) => {
            mark_session_failed(db, upload_id).await;
            Err(e)
        }
    }
}

/// 完成 S3 multipart presigned 上传：complete multipart → hash → 去重 → copy → 建文件记录
async fn complete_s3_multipart(
    state: &AppState,
    session: upload_session::Model,
    mut parts: Vec<(i32, String)>,
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

    let policy = policy_repo::find_by_id(db, session.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    // ── 原子状态转换 presigned → assembling（防止并发 complete 双重触发） ──
    let upload_id = &session.id;
    let transitioned = upload_session_repo::try_transition_status(
        db,
        upload_id,
        UploadSessionStatus::Presigned,
        UploadSessionStatus::Assembling,
    )
    .await?;
    if !transitioned {
        return Err(AsterError::upload_assembly_failed(format!(
            "session status is '{:?}', expected 'presigned'",
            session.status
        )));
    }

    // ── 完成 S3 multipart → finalize（不做 SHA256，S3 不去重） ──
    let result = async {
        // 完成 S3 multipart upload（parts 按 part_number 排序）
        parts.sort_by_key(|(num, _)| *num);
        driver
            .complete_multipart_upload(&temp_key, &multipart_id, parts)
            .await?;

        // 验证文件大小
        let meta = driver.metadata(&temp_key).await.map_err(|_| {
            AsterError::upload_assembly_failed(
                "S3 object not found after multipart complete - assembly may have failed",
            )
        })?;
        let actual_size = meta.size as i64;

        if actual_size != session.total_size {
            if let Err(e) = driver.delete(&temp_key).await {
                tracing::warn!("failed to delete S3 temp object: {e}");
            }
            return Err(AsterError::upload_assembly_failed(format!(
                "size mismatch: declared {} but uploaded {}",
                session.total_size, actual_size
            )));
        }

        let file_hash = format!("s3-{}", upload_id); // 占位 hash，S3 不去重
        let now = Utc::now();

        let created = finalize_upload_session(
            state,
            &session,
            &file_hash,
            actual_size,
            policy.id,
            &temp_key, // 直接用 temp_key，无需 copy
            now,
        )
        .await?;

        Ok(created)
    }
    .await;

    match result {
        Ok(f) => Ok(f),
        Err(e) => {
            mark_session_failed(db, upload_id).await;
            Err(e)
        }
    }
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

/// 上传完成的事务内共用逻辑：blob 去重 → 文件记录 → 配额 → session 状态
async fn finalize_upload_session(
    state: &AppState,
    session: &upload_session::Model,
    file_hash: &str,
    size: i64,
    policy_id: i64,
    storage_path: &str,
    now: chrono::DateTime<Utc>,
) -> Result<file::Model> {
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    let blob =
        file_repo::find_or_create_blob(&txn, file_hash, size, policy_id, storage_path).await?;

    let final_name = file_repo::resolve_unique_filename(
        &txn,
        session.user_id,
        session.folder_id,
        &session.filename,
    )
    .await?;

    let mime = mime_guess::from_path(&final_name)
        .first_or_octet_stream()
        .to_string();

    let created = file_repo::create(
        &txn,
        file::ActiveModel {
            name: Set(final_name),
            folder_id: Set(session.folder_id),
            blob_id: Set(blob.id),
            size: Set(blob.size),
            user_id: Set(session.user_id),
            mime_type: Set(mime),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    user_repo::update_storage_used(&txn, session.user_id, size).await?;

    let session_fresh = upload_session_repo::find_by_id(&txn, &session.id).await?;
    let mut active: upload_session::ActiveModel = session_fresh.into();
    active.status = Set(UploadSessionStatus::Completed);
    active.file_id = Set(Some(created.id));
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(&txn, active).await?;

    txn.commit().await.map_err(AsterError::from)?;
    Ok(created)
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
pub async fn cancel_upload(state: &AppState, upload_id: &str, user_id: i64) -> Result<()> {
    let session = upload_session_repo::find_by_id(&state.db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;

    // 清理 S3 临时对象 / multipart upload
    if let Some(ref temp_key) = session.s3_temp_key {
        let policy = policy_repo::find_by_id(&state.db, session.policy_id).await?;
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

    let temp_dir = format!("data/.uploads/{upload_id}");
    crate::utils::cleanup_temp_dir(&temp_dir).await;
    upload_session_repo::delete(&state.db, upload_id).await
}

/// 查询上传进度
pub async fn get_progress(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
) -> Result<UploadProgressResponse> {
    let session = upload_session_repo::find_by_id(&state.db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;

    // S3 multipart → 从 S3 查询已上传 parts；本地分片 → 扫磁盘
    let chunks_on_disk = if let (Some(temp_key), Some(multipart_id)) =
        (&session.s3_temp_key, &session.s3_multipart_id)
    {
        let policy = policy_repo::find_by_id(&state.db, session.policy_id).await?;
        let driver = state.driver_registry.get_driver(&policy)?;
        driver.list_uploaded_parts(temp_key, multipart_id).await?
    } else {
        scan_received_chunks(&session.id).await
    };

    Ok(UploadProgressResponse {
        upload_id: session.id,
        status: session.status,
        received_count: session.received_count,
        chunks_on_disk,
        total_chunks: session.total_chunks,
        filename: session.filename,
    })
}

/// 为 S3 multipart presigned 上传批量生成 per-part presigned PUT URL
pub async fn presign_parts(
    state: &AppState,
    upload_id: &str,
    user_id: i64,
    part_numbers: Vec<i32>,
) -> Result<std::collections::HashMap<i32, String>> {
    let db = &state.db;
    let session = upload_session_repo::find_by_id(db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;

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

    let policy = policy_repo::find_by_id(db, session.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let expires = std::time::Duration::from_secs(HOUR_SECS);
    let mut urls = std::collections::HashMap::new();
    for part_num in part_numbers {
        let url = driver
            .presigned_upload_part_url(temp_key, multipart_id, part_num, expires)
            .await?;
        urls.insert(part_num, url);
    }
    Ok(urls)
}

/// 扫描临时目录中实际存在的 chunk 文件，返回排序后的 chunk 编号列表
async fn scan_received_chunks(upload_id: &str) -> Vec<i32> {
    let dir = format!("data/.uploads/{upload_id}");
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
            && let Ok(policy) = policy_repo::find_by_id(&state.db, session.policy_id).await
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
        let temp_dir = format!("data/.uploads/{}", session.id);
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
