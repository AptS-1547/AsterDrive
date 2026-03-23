use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::repository::{file_repo, policy_repo, upload_session_repo, user_repo};
use crate::entities::{file, file_blob, upload_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::file_service;
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

/// 上传协商：服务端根据存储策略决定上传模式
pub async fn init_upload(
    state: &AppState,
    user_id: i64,
    filename: &str,
    total_size: i64,
    folder_id: Option<i64>,
) -> Result<InitUploadResponse> {
    let db = &state.db;

    // 确定存储策略
    let policy = file_service::resolve_policy(state, user_id, folder_id).await?;

    // 检查文件大小限制
    if policy.max_file_size > 0 && total_size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            total_size, policy.max_file_size
        )));
    }

    // 检查用户配额
    let user = user_repo::find_by_id(db, user_id).await?;
    if user.storage_quota > 0 && user.storage_used + total_size > user.storage_quota {
        return Err(AsterError::storage_quota_exceeded(format!(
            "quota {}, used {}, need {}",
            user.storage_quota, user.storage_used, total_size
        )));
    }

    // S3 presigned 直传：策略开启 + S3 驱动 + 文件 ≤ 5GiB
    const S3_SINGLE_PUT_LIMIT: i64 = 5 * 1024 * 1024 * 1024; // 5 GiB
    if policy.driver_type == DriverType::S3 && total_size <= S3_SINGLE_PUT_LIMIT {
        let opts = parse_policy_options(&policy.options);
        if opts.presigned_upload {
            let driver = state.driver_registry.get_driver(&policy)?;
            let upload_id = id::new_uuid();
            let temp_key = format!("_tmp/{upload_id}");
            let presigned_url = driver
                .presigned_put_url(&temp_key, std::time::Duration::from_secs(3600))
                .await?
                .ok_or_else(|| {
                    AsterError::storage_driver_error("presigned PUT not supported by driver")
                })?;

            let now = Utc::now();
            let expires_at = now + chrono::Duration::hours(1);

            let session = upload_session::ActiveModel {
                id: Set(upload_id.clone()),
                user_id: Set(user_id),
                filename: Set(filename.to_string()),
                total_size: Set(total_size),
                chunk_size: Set(0),
                total_chunks: Set(0),
                received_count: Set(0),
                folder_id: Set(folder_id),
                policy_id: Set(policy.id),
                status: Set(UploadSessionStatus::Presigned),
                s3_temp_key: Set(Some(temp_key)),
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
    let upload_id = id::new_uuid();
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
        filename: Set(filename.to_string()),
        total_size: Set(total_size),
        chunk_size: Set(chunk_size),
        total_chunks: Set(total_chunks),
        received_count: Set(0),
        folder_id: Set(folder_id),
        policy_id: Set(policy.id),
        status: Set(UploadSessionStatus::Uploading),
        s3_temp_key: Set(None),
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
) -> Result<file::Model> {
    let db = &state.db;
    let session = upload_session_repo::find_by_id(db, upload_id).await?;

    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;
    // Presigned 模式走独立流程
    if session.status == UploadSessionStatus::Presigned {
        return complete_presigned_upload(state, session).await;
    }

    if session.status != UploadSessionStatus::Uploading {
        return Err(AsterError::upload_assembly_failed(format!(
            "session status is '{:?}', expected 'uploading' or 'presigned'",
            session.status
        )));
    }

    if session.received_count != session.total_chunks {
        return Err(AsterError::upload_assembly_failed(format!(
            "expected {} chunks, got {}",
            session.total_chunks, session.received_count
        )));
    }

    // ── [事务外] 标记为 assembling ──
    let mut active: upload_session::ActiveModel = session.clone().into();
    active.status = Set(UploadSessionStatus::Assembling);
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(db, active).await?;

    // ── [事务外] 流式拼接分片 + sha256 ──
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let assembled_path = format!("data/.uploads/{upload_id}/_assembled");
    let mut out_file = tokio::fs::File::create(&assembled_path)
        .await
        .map_aster_err_ctx("create assembled file", AsterError::upload_assembly_failed)?;
    let mut hasher = Sha256::new();
    let mut size: i64 = 0;

    for i in 0..session.total_chunks {
        let chunk_path = format!("data/.uploads/{upload_id}/chunk_{i}");
        let chunk_data = tokio::fs::read(&chunk_path)
            .await
            .map_err(|e| AsterError::upload_assembly_failed(format!("read chunk {i}: {e}")))?;
        hasher.update(&chunk_data);
        size += chunk_data.len() as i64;
        out_file
            .write_all(&chunk_data)
            .await
            .map_aster_err_ctx("write assembled", AsterError::upload_assembly_failed)?;
    }
    out_file
        .flush()
        .await
        .map_aster_err_ctx("flush assembled", AsterError::upload_assembly_failed)?;
    drop(out_file);

    let file_hash = format!("{:x}", hasher.finalize());
    let now = Utc::now();

    // ── [事务外] 获取策略 + driver + put_file ──
    let policy = policy_repo::find_by_id(db, session.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let storage_path = format!("{}/{}/{}", &file_hash[..2], &file_hash[2..4], &file_hash);
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

    // ── [事务内] blob 查找/创建 → 文件记录创建 → 配额更新 → session 状态更新 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // Blob 去重（事务内重新检查，防止并发竞争）
    let blob = match file_repo::find_blob_by_hash(&txn, &file_hash, policy.id).await? {
        Some(existing) => {
            let new_ref_count = existing.ref_count + 1;
            let mut blob_active: file_blob::ActiveModel = existing.into();
            blob_active.ref_count = Set(new_ref_count);
            blob_active.update(&txn).await.map_err(AsterError::from)?
        }
        None => {
            let blob_model = file_blob::ActiveModel {
                hash: Set(file_hash),
                size: Set(size),
                ref_count: Set(1),
                policy_id: Set(policy.id),
                storage_path: Set(storage_path),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            file_repo::create_blob(&txn, blob_model).await?
        }
    };

    // 检查同名文件（事务内检查保证一致性）
    if file_repo::find_by_name_in_folder(
        &txn,
        session.user_id,
        session.folder_id,
        &session.filename,
    )
    .await?
    .is_some()
    {
        // txn drop 自动 rollback
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in this folder",
            session.filename
        )));
    }

    let mime = mime_guess::from_path(&session.filename)
        .first_or_octet_stream()
        .to_string();

    let file_model = file::ActiveModel {
        name: Set(session.filename.clone()),
        folder_id: Set(session.folder_id),
        blob_id: Set(blob.id),
        size: Set(blob.size),
        user_id: Set(session.user_id),
        mime_type: Set(mime),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let created = file_repo::create(&txn, file_model).await?;

    // 配额更新
    user_repo::update_storage_used(&txn, session.user_id, size).await?;

    // session 状态更新
    let session_fresh = upload_session_repo::find_by_id(&txn, upload_id).await?;
    let mut active: upload_session::ActiveModel = session_fresh.into();
    active.status = Set(UploadSessionStatus::Completed);
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(&txn, active).await?;

    txn.commit().await.map_err(AsterError::from)?;

    // ── [事务外] 清理临时文件 ──
    let temp_dir = format!("data/.uploads/{upload_id}");
    crate::utils::cleanup_temp_dir(&temp_dir).await;

    Ok(created)
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

    // ── [事务外] 标记 assembling ──
    let mut active: upload_session::ActiveModel = session.clone().into();
    active.status = Set(UploadSessionStatus::Assembling);
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(db, active).await?;

    // ── [事务外] 流式 SHA256（从 S3 读，64KB buffer） ──
    let file_hash = {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;
        let mut hasher = Sha256::new();
        let mut stream = driver.get_stream(&temp_key).await?;
        let mut buf = vec![0u8; 65536];
        loop {
            let n = stream
                .read(&mut buf)
                .await
                .map_aster_err_ctx("read S3 stream", AsterError::upload_assembly_failed)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        format!("{:x}", hasher.finalize())
    };

    let now = Utc::now();

    // ── [事务外] copy_object（仅新 blob 时需要） ──
    let storage_path = format!("{}/{}/{}", &file_hash[..2], &file_hash[2..4], &file_hash);
    let blob_pre_exists = file_repo::find_blob_by_hash(db, &file_hash, policy.id)
        .await?
        .is_some();
    if !blob_pre_exists {
        driver.copy_object(&temp_key, &storage_path).await?;
    }

    // ── [事务内] blob 查找/创建 → 文件记录创建 → 配额更新 → session 状态更新 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // Blob 去重（事务内重新检查，防止并发竞争）
    let blob = match file_repo::find_blob_by_hash(&txn, &file_hash, policy.id).await? {
        Some(existing) => {
            let new_ref_count = existing.ref_count + 1;
            let mut blob_active: file_blob::ActiveModel = existing.into();
            blob_active.ref_count = Set(new_ref_count);
            blob_active.updated_at = Set(now);
            blob_active.update(&txn).await.map_err(AsterError::from)?
        }
        None => {
            let blob_model = file_blob::ActiveModel {
                hash: Set(file_hash),
                size: Set(actual_size),
                ref_count: Set(1),
                policy_id: Set(policy.id),
                storage_path: Set(storage_path),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            file_repo::create_blob(&txn, blob_model).await?
        }
    };

    // 检查同名文件（事务内检查保证一致性）
    if file_repo::find_by_name_in_folder(
        &txn,
        session.user_id,
        session.folder_id,
        &session.filename,
    )
    .await?
    .is_some()
    {
        // txn drop 自动 rollback
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in this folder",
            session.filename
        )));
    }

    let mime = mime_guess::from_path(&session.filename)
        .first_or_octet_stream()
        .to_string();

    let file_model = file::ActiveModel {
        name: Set(session.filename.clone()),
        folder_id: Set(session.folder_id),
        blob_id: Set(blob.id),
        size: Set(blob.size),
        user_id: Set(session.user_id),
        mime_type: Set(mime),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let created = file_repo::create(&txn, file_model).await?;

    // 配额更新
    user_repo::update_storage_used(&txn, session.user_id, actual_size).await?;

    // session 状态更新
    let session_fresh = upload_session_repo::find_by_id(&txn, &session.id).await?;
    let mut active: upload_session::ActiveModel = session_fresh.into();
    active.status = Set(UploadSessionStatus::Completed);
    active.updated_at = Set(Utc::now());
    upload_session_repo::update(&txn, active).await?;

    txn.commit().await.map_err(AsterError::from)?;

    // ── [事务外] S3 临时对象清理（best-effort） ──
    if let Err(e) = driver.delete(&temp_key).await {
        tracing::warn!("failed to delete S3 temp object: {e}");
    }

    Ok(created)
}

/// 取消上传
pub async fn cancel_upload(state: &AppState, upload_id: &str, user_id: i64) -> Result<()> {
    let session = upload_session_repo::find_by_id(&state.db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, user_id, "upload session")?;

    // 清理 S3 临时对象
    if let Some(ref temp_key) = session.s3_temp_key {
        let policy = policy_repo::find_by_id(&state.db, session.policy_id).await?;
        if let Ok(driver) = state.driver_registry.get_driver(&policy)
            && let Err(e) = driver.delete(temp_key).await
        {
            tracing::warn!("failed to delete S3 temp object: {e}");
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

    // 扫磁盘获取具体哪些 chunk 存在（用于断点续传判断缺哪些）
    let chunks_on_disk = scan_received_chunks(&session.id).await;

    Ok(UploadProgressResponse {
        upload_id: session.id,
        status: session.status,
        received_count: session.received_count,
        chunks_on_disk,
        total_chunks: session.total_chunks,
        filename: session.filename,
    })
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
        // 清理 S3 临时对象
        if let Some(ref temp_key) = session.s3_temp_key
            && let Ok(policy) = policy_repo::find_by_id(&state.db, session.policy_id).await
            && let Ok(driver) = state.driver_registry.get_driver(&policy)
            && let Err(e) = driver.delete(temp_key).await
        {
            tracing::warn!("failed to delete S3 temp object: {e}");
        }
        let temp_dir = format!("data/.uploads/{}", session.id);
        crate::utils::cleanup_temp_dir(&temp_dir).await;
        let _ = upload_session_repo::delete(&state.db, &session.id).await;
    }
    if count > 0 {
        tracing::info!("cleaned up {count} expired upload sessions");
    }
    Ok(count)
}
