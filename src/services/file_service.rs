use actix_multipart::Multipart;
use actix_web::HttpResponse;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use tokio::io::AsyncWriteExt;

use crate::cache::CacheExt;
use crate::db::repository::{file_repo, policy_repo, user_repo};
use crate::entities::{file, file_blob, user_storage_policy};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::thumbnail_service;
use crate::types::{
    NullablePatch, S3UploadStrategy, effective_s3_multipart_chunk_size,
    parse_storage_policy_options,
};

const HASH_BUF_SIZE: usize = 65536; // 64KB

async fn create_s3_nondedup_file(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
    size: i64,
    policy_id: i64,
    storage_path: &str,
    file_hash: &str,
) -> Result<file::Model> {
    let now = Utc::now();
    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    user_repo::check_quota(&txn, user_id, size).await?;

    let blob =
        file_repo::find_or_create_blob(&txn, file_hash, size, policy_id, storage_path).await?;
    let final_name = file_repo::resolve_unique_filename(&txn, user_id, folder_id, filename).await?;

    let created = file_repo::create(
        &txn,
        file::ActiveModel {
            name: Set(final_name),
            folder_id: Set(folder_id),
            blob_id: Set(blob.model.id),
            size: Set(blob.model.size),
            user_id: Set(user_id),
            mime_type: Set(mime),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    user_repo::update_storage_used(&txn, user_id, size).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(created)
}

async fn relay_field_to_s3(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
    mut field: actix_multipart::Field,
    policy: &crate::entities::storage_policy::Model,
) -> Result<file::Model> {
    let driver = state.driver_registry.get_driver(policy)?;
    let upload_id = crate::utils::id::new_uuid();
    let storage_path = format!("files/{upload_id}");
    let multipart_id = driver.create_multipart_upload(&storage_path).await?;
    let part_size = effective_s3_multipart_chunk_size(policy.chunk_size) as usize;

    let result = async {
        let mut total_size: i64 = 0;
        let mut part_number = 1;
        let mut uploaded_parts = Vec::new();
        let mut buffer = Vec::with_capacity(part_size);

        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
            total_size += chunk.len() as i64;
            if policy.max_file_size > 0 && total_size > policy.max_file_size {
                return Err(AsterError::file_too_large(format!(
                    "file size {} exceeds limit {}",
                    total_size, policy.max_file_size
                )));
            }

            buffer.extend_from_slice(&chunk);

            while buffer.len() >= part_size {
                let remainder = buffer.split_off(part_size);
                let current_part = std::mem::replace(&mut buffer, remainder);
                let etag = driver
                    .upload_multipart_part(&storage_path, &multipart_id, part_number, &current_part)
                    .await?;
                uploaded_parts.push((part_number, etag));
                part_number += 1;
            }
        }

        if total_size == 0 {
            return Err(AsterError::validation_error("empty file"));
        }

        user_repo::check_quota(&state.db, user_id, total_size).await?;

        if !buffer.is_empty() || uploaded_parts.is_empty() {
            let final_part_etag = driver
                .upload_multipart_part(&storage_path, &multipart_id, part_number, &buffer)
                .await?;
            uploaded_parts.push((part_number, final_part_etag));
        }

        driver
            .complete_multipart_upload(&storage_path, &multipart_id, uploaded_parts)
            .await?;

        let file_hash = format!("s3-{upload_id}");
        create_s3_nondedup_file(
            state,
            user_id,
            folder_id,
            filename,
            total_size,
            policy.id,
            &storage_path,
            &file_hash,
        )
        .await
    }
    .await;

    match result {
        Ok(file) => Ok(file),
        Err(err) => {
            if let Err(cleanup_err) = driver
                .abort_multipart_upload(&storage_path, &multipart_id)
                .await
            {
                tracing::warn!("failed to abort relay multipart upload {upload_id}: {cleanup_err}");
                if let Err(delete_err) = driver.delete(&storage_path).await {
                    tracing::warn!(
                        "failed to delete relay object {storage_path} after abort failure: {delete_err}"
                    );
                }
            }
            Err(err)
        }
    }
}

/// 从临时文件存储 blob 并创建文件记录
///
/// 公共函数，REST upload 和 WebDAV flush 都调用。
/// - 流式 sha256（不加载全文件到内存）
/// - 策略检查 + 配额检查 + blob 去重
/// - `put_file` 零拷贝（LocalDriver rename）
/// - 创建/覆盖文件记录
///
/// `existing_file_id`: Some 时覆盖现有文件，None 时新建
///
/// 返回创建/更新的文件记录。临时文件可能被 put_file rename 走，调用方不要依赖它存在。
/// `skip_lock_check`: WebDAV 持锁者写入时为 true（dav-server 已验证 lock token）
pub async fn store_from_temp(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
    skip_lock_check: bool,
) -> Result<file::Model> {
    let db = &state.db;

    crate::utils::validate_name(filename)?;

    // ── [事务外] sha256 计算 ──
    let file_hash = {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;
        let mut hasher = Sha256::new();
        let mut reader = tokio::fs::File::open(temp_path)
            .await
            .map_aster_err_ctx("open temp", AsterError::file_upload_failed)?;
        let mut buf = vec![0u8; HASH_BUF_SIZE];
        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_aster_err_ctx("read temp", AsterError::file_upload_failed)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
    };

    // ── [事务外] 策略解析 ──
    let policy = resolve_policy(state, user_id, folder_id).await?;

    // 文件大小限制
    if policy.max_file_size > 0 && size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            size, policy.max_file_size
        )));
    }

    // ── [事务外] 配额检查 ──
    user_repo::check_quota(db, user_id, size).await?;

    let now = Utc::now();
    let driver = state.driver_registry.get_driver(&policy)?;

    let storage_path = crate::utils::storage_path_from_hash(&file_hash);

    // ── [事务外] 覆盖模式：预读旧文件 + 删除旧缩略图 ──
    let overwrite_ctx = if let Some(existing_id) = existing_file_id {
        let old_file = file_repo::find_by_id(db, existing_id).await?;
        if old_file.is_locked && !skip_lock_check {
            return Err(AsterError::resource_locked("file is locked"));
        }
        let old_blob = file_repo::find_blob_by_id(db, old_file.blob_id).await?;
        // 覆盖时删除旧缩略图（新 blob 的缩略图会按需生成）
        if let Err(e) = crate::services::thumbnail_service::delete_thumbnail(state, &old_blob).await
        {
            tracing::warn!("failed to delete thumbnail for blob {}: {e}", old_blob.id);
        }
        Some((old_file, old_blob))
    } else {
        None
    };

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    // ── [事务内] 配额再校验 → blob 查找/创建(ref_count) → 文件记录创建/更新 → 版本记录 → 配额更新 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 事务内配额权威检查（防止并发上传绕过事务外 fast-fail）
    user_repo::check_quota(&txn, user_id, size).await?;

    // Blob 去重（事务内重新检查，防止并发竞争）
    let blob =
        file_repo::find_or_create_blob(&txn, &file_hash, size, policy.id, &storage_path).await?;
    if blob.inserted {
        driver.put_file(&storage_path, temp_path).await?;
    }

    let result = if let Some((old_file, old_blob)) = overwrite_ctx {
        // 覆盖现有文件
        let existing_id = old_file.id;
        let mut active: file::ActiveModel = old_file.into();
        active.blob_id = Set(blob.model.id);
        active.size = Set(blob.model.size);
        active.mime_type = Set(mime);
        active.updated_at = Set(now);
        let updated = active.update(&txn).await.map_err(AsterError::from)?;

        // 版本溯源：保留旧 blob 作为历史版本（不减 ref_count）
        let next_ver = crate::db::repository::version_repo::next_version(&txn, existing_id).await?;
        crate::db::repository::version_repo::create(
            &txn,
            crate::entities::file_version::ActiveModel {
                file_id: Set(existing_id),
                blob_id: Set(old_blob.id),
                version: Set(next_ver),
                size: Set(old_blob.size),
                created_at: Set(now),
                ..Default::default()
            },
        )
        .await?;

        // 配额：只增加新文件大小（旧版本 blob 已计入配额）
        user_repo::update_storage_used(&txn, user_id, size).await?;

        updated
    } else {
        // 新建文件
        let final_name =
            file_repo::resolve_unique_filename(&txn, user_id, folder_id, filename).await?;

        let file_model = file::ActiveModel {
            name: Set(final_name),
            folder_id: Set(folder_id),
            blob_id: Set(blob.model.id),
            size: Set(blob.model.size),
            user_id: Set(user_id),
            mime_type: Set(mime),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        let created = file_repo::create(&txn, file_model).await?;
        user_repo::update_storage_used(&txn, user_id, size).await?;

        created
    };

    txn.commit().await.map_err(AsterError::from)?;

    // ── [事务后] 清理超出上限的旧版本（独立操作，不需要在主事务内） ──
    if let Some(existing_id) = existing_file_id {
        crate::services::version_service::cleanup_excess(state, existing_id).await?;
    }

    Ok(result)
}

/// 上传文件（REST API，multipart）
pub async fn upload(
    state: &AppState,
    user_id: i64,
    payload: &mut Multipart,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
) -> Result<file::Model> {
    let (resolved_folder_id, resolved_filename) = match relative_path {
        Some(path) => {
            crate::services::folder_service::resolve_upload_path(state, user_id, folder_id, path)
                .await?
        }
        None => {
            if let Some(fid) = folder_id {
                crate::services::folder_service::verify_folder_access(state, user_id, fid).await?;
            }
            (folder_id, String::new())
        }
    };

    let effective_folder_id = if relative_path.is_some() {
        resolved_folder_id
    } else {
        folder_id
    };
    let policy = resolve_policy(state, user_id, effective_folder_id).await?;
    let policy_options = parse_storage_policy_options(&policy.options);
    let s3_strategy = if policy.driver_type == crate::types::DriverType::S3 {
        Some(policy_options.effective_s3_upload_strategy())
    } else {
        None
    };

    if s3_strategy == Some(S3UploadStrategy::RelayStream) {
        while let Some(field) = payload.next().await {
            let field = field.map_aster_err(AsterError::file_upload_failed)?;
            let uploaded_name = field
                .content_disposition()
                .and_then(|cd| cd.get_filename().map(|n| n.to_string()));

            if let Some(name) = uploaded_name {
                let filename = if relative_path.is_some() {
                    resolved_filename.clone()
                } else {
                    name
                };
                crate::utils::validate_name(&filename)?;
                return relay_field_to_s3(
                    state,
                    user_id,
                    effective_folder_id,
                    &filename,
                    field,
                    &policy,
                )
                .await;
            }
        }

        return Err(AsterError::validation_error("empty file"));
    }

    // 流式写入临时文件（不在内存中缓冲整个文件）
    let mut filename = String::from("unnamed");
    let temp_path = format!("{}/{}", crate::utils::TEMP_DIR, uuid::Uuid::new_v4());
    tokio::fs::create_dir_all(crate::utils::TEMP_DIR)
        .await
        .map_aster_err_ctx("create temp dir", AsterError::file_upload_failed)?;

    let mut temp_file = tokio::fs::File::create(&temp_path)
        .await
        .map_aster_err_ctx("create temp", AsterError::file_upload_failed)?;
    let mut size: i64 = 0;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let is_file = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|n| n.to_string()));

        if let Some(name) = is_file {
            filename = if relative_path.is_some() {
                resolved_filename.clone()
            } else {
                name
            };
            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_aster_err_ctx("write temp", AsterError::file_upload_failed)?;
                size += chunk.len() as i64;
            }
        }
    }

    temp_file
        .flush()
        .await
        .map_aster_err_ctx("flush temp", AsterError::file_upload_failed)?;
    drop(temp_file);

    if size == 0 {
        crate::utils::cleanup_temp_file(&temp_path).await;
        return Err(AsterError::validation_error("empty file"));
    }

    let result = store_from_temp(
        state,
        user_id,
        effective_folder_id,
        &filename,
        &temp_path,
        size,
        None,
        false,
    )
    .await;

    // 清理临时文件（put_file 可能已经 rename 走了，忽略错误）
    crate::utils::cleanup_temp_file(&temp_path).await;

    result
}

/// 获取文件信息
pub async fn get_info(state: &AppState, id: i64, user_id: i64) -> Result<file::Model> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{id} is in trash"
        )));
    }
    Ok(f)
}

/// 下载文件（流式，不全量缓冲）
pub async fn download(
    state: &AppState,
    id: i64,
    user_id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{id} is in trash"
        )));
    }

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    build_stream_response(state, &f, &blob, if_none_match).await
}

/// 下载文件（无用户校验，用于分享链接，流式）
pub async fn download_raw(
    state: &AppState,
    id: i64,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    build_stream_response(state, &f, &blob, if_none_match).await
}

fn if_none_match_matches(if_none_match: &str, blob_hash: &str) -> bool {
    if_none_match.split(',').any(|value| {
        let candidate = value.trim();
        candidate == "*" || candidate.trim_matches('"').eq_ignore_ascii_case(blob_hash)
    })
}

/// 构建流式下载响应
async fn build_stream_response(
    state: &AppState,
    f: &file::Model,
    blob: &file_blob::Model,
    if_none_match: Option<&str>,
) -> Result<HttpResponse> {
    let etag = format!("\"{}\"", blob.hash);
    if let Some(if_none_match) = if_none_match
        && if_none_match_matches(if_none_match, &blob.hash)
    {
        return Ok(HttpResponse::NotModified()
            .insert_header(("ETag", etag))
            .insert_header(("Cache-Control", "private, max-age=0, must-revalidate"))
            .finish());
    }

    let policy = policy_repo::find_by_id(&state.db, blob.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let stream = driver.get_stream(&blob.storage_path).await?;

    // 64KB buffer — 比默认 4KB 减少系统调用和分配开销
    let reader_stream = tokio_util::io::ReaderStream::with_capacity(stream, 64 * 1024);

    Ok(HttpResponse::Ok()
        .content_type(f.mime_type.clone())
        .insert_header(("Content-Length", blob.size.to_string()))
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", f.name),
        ))
        .insert_header(("ETag", etag))
        .insert_header(("Cache-Control", "private, max-age=0, must-revalidate"))
        // 跳过全局 Compress 中间件，避免压缩编码器缓冲导致内存暴涨
        .insert_header(("Content-Encoding", "identity"))
        .streaming(reader_stream))
}

/// 删除文件（软删除 → 回收站）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }
    file_repo::soft_delete(&state.db, id).await
}

pub(crate) async fn cleanup_unreferenced_blob(state: &AppState, blob: &file_blob::Model) -> bool {
    async fn restore_cleanup_claim(state: &AppState, blob_id: i64, reason: &str) {
        match file_repo::restore_blob_cleanup_claim(&state.db, blob_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    blob_id,
                    "blob cleanup claim was already released while handling {reason}"
                );
            }
            Err(e) => {
                tracing::warn!(
                    blob_id,
                    "failed to restore blob cleanup claim after {reason}: {e}"
                );
            }
        }
    }

    let current_blob = match file_repo::find_blob_by_id(&state.db, blob.id).await {
        Ok(current_blob) => current_blob,
        Err(e) if e.code() == "E006" => return true,
        Err(e) => {
            tracing::warn!(
                blob_id = blob.id,
                "failed to reload blob before cleanup: {e}"
            );
            return false;
        }
    };

    if current_blob.ref_count != 0 {
        tracing::warn!(
            blob_id = current_blob.id,
            ref_count = current_blob.ref_count,
            "skipping blob cleanup because blob is referenced again"
        );
        return false;
    }

    match file_repo::claim_blob_cleanup(&state.db, current_blob.id).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "skipping blob cleanup because another worker already claimed it or it was revived"
            );
            return false;
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "failed to claim blob cleanup: {e}"
            );
            return false;
        }
    }

    if let Err(e) = thumbnail_service::delete_thumbnail(state, &current_blob).await {
        tracing::warn!(
            blob_id = current_blob.id,
            "failed to delete thumbnail during blob cleanup: {e}"
        );
    }

    let policy = match policy_repo::find_by_id(&state.db, current_blob.policy_id).await {
        Ok(policy) => policy,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                policy_id = current_blob.policy_id,
                "failed to load storage policy during blob cleanup: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "policy lookup failure").await;
            return false;
        }
    };

    let driver = match state.driver_registry.get_driver(&policy) {
        Ok(driver) => driver,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                policy_id = current_blob.policy_id,
                "failed to resolve storage driver during blob cleanup: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "driver resolution failure").await;
            return false;
        }
    };

    let object_deleted = match driver.delete(&current_blob.storage_path).await {
        Ok(()) => true,
        Err(e) => match driver.exists(&current_blob.storage_path).await {
            Ok(false) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "blob delete returned error but object is already absent: {e}"
                );
                true
            }
            Ok(true) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object, keeping blob row for retry: {e}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete error").await;
                false
            }
            Err(exists_err) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object and verify existence, keeping blob row for retry: delete_error={e}, exists_error={exists_err}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete verification error").await;
                false
            }
        },
    };

    if !object_deleted {
        return false;
    }

    match file_repo::delete_blob_if_cleanup_claimed(&state.db, current_blob.id).await {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but cleanup claim was lost before deleting blob row"
            );
            restore_cleanup_claim(
                state,
                current_blob.id,
                "lost cleanup claim before row delete",
            )
            .await;
            false
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but failed to delete blob row: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "row delete failure").await;
            false
        }
    }
}

/// 永久删除文件（回收站清理用，处理 blob ref_count + 物理文件 + 缩略图 + 配额）
pub async fn purge(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    // 注意：不检查 is_locked——回收站内的文件和 recursive_purge_folder 都需要无条件清理

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    let blob_size = blob.size;

    // ── [事务内] 版本清理 → 属性删除 → 文件删除 → blob ref_count-- → 配额更新 ──
    // 注意：文件删除必须在 blob 删除之前（files.blob_id → file_blobs.id FK 约束）
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 版本清理：删除版本记录（file_versions 无 FK 到 file_blobs，安全）
    let version_blob_ids =
        crate::db::repository::version_repo::delete_all_by_file_id(&txn, id).await?;

    // 属性删除
    crate::db::repository::property_repo::delete_all_for_entity(
        &txn,
        crate::types::EntityType::File,
        id,
    )
    .await?;

    // 文件删除（先于 blob 删除，解除 FK 引用）
    file_repo::delete(&txn, id).await?;

    // 版本 blob / 主 blob 引用处理（文件和版本记录已删除，可安全操作 blob）
    let mut blob_decrements = std::collections::HashMap::<i64, i32>::new();
    for vblob_id in version_blob_ids {
        *blob_decrements.entry(vblob_id).or_default() += 1;
    }
    *blob_decrements.entry(blob.id).or_default() += 1;

    let blob_ids: Vec<i64> = blob_decrements.keys().copied().collect();
    let blobs_by_id = file_repo::find_blobs_by_ids(&txn, &blob_ids).await?;
    let mut blobs_to_cleanup: Vec<file_blob::Model> = Vec::new();
    for (blob_id, decrement) in blob_decrements {
        let Some(current_blob) = blobs_by_id.get(&blob_id) else {
            continue;
        };

        file_repo::decrement_blob_ref_count_by(&txn, blob_id, decrement).await?;
        if current_blob.ref_count <= decrement {
            blobs_to_cleanup.push(current_blob.clone());
        }
    }

    // 配额更新
    user_repo::update_storage_used(&txn, user_id, -blob_size).await?;

    txn.commit().await.map_err(AsterError::from)?;

    // ── [事务后] 物理文件清理成功后再删 blob 元数据 ──
    for blob_to_clean in blobs_to_cleanup {
        if !cleanup_unreferenced_blob(state, &blob_to_clean).await {
            tracing::warn!(
                blob_id = blob_to_clean.id,
                "blob cleanup incomplete after purge; blob row retained for retry"
            );
        }
    }

    Ok(())
}

/// 批量永久删除文件：一次事务处理所有 DB 操作，事务后并行清理物理文件
///
/// 比逐个调 `purge()` 快得多——N 个文件只需 ~10 次 DB 查询而非 ~12N 次。
pub async fn batch_purge(state: &AppState, files: Vec<file::Model>, user_id: i64) -> Result<u32> {
    if files.is_empty() {
        return Ok(0);
    }

    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let blob_ids: Vec<i64> = files.iter().map(|f| f.blob_id).collect();
    let total_size: i64 = files.iter().map(|f| f.size).sum();
    let count = files.len() as u32;

    // ── 单次事务：版本 → 属性 → 文件 → blob → 配额 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 1. 批量删除版本记录，收集版本 blob IDs
    let version_blob_ids =
        crate::db::repository::version_repo::delete_all_by_file_ids(&txn, &file_ids).await?;

    // 2. 批量删除文件属性
    crate::db::repository::property_repo::delete_all_for_entities(
        &txn,
        crate::types::EntityType::File,
        &file_ids,
    )
    .await?;

    // 3. 批量删除文件记录（先于 blob，解除 FK）
    file_repo::delete_many(&txn, &file_ids).await?;

    // 4. 处理 blob 引用计数
    //    合并主 blob 和版本 blob，按 blob_id 统计需要减少的引用数
    let mut blob_decrements: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &bid in &blob_ids {
        *blob_decrements.entry(bid).or_default() += 1;
    }
    for &vbid in &version_blob_ids {
        *blob_decrements.entry(vbid).or_default() += 1;
    }

    let blob_ids: Vec<i64> = blob_decrements.keys().copied().collect();
    let blobs_by_id = file_repo::find_blobs_by_ids(&txn, &blob_ids).await?;
    let mut blobs_to_cleanup: Vec<file_blob::Model> = Vec::new();

    for (&blob_id, &decrement) in &blob_decrements {
        if let Some(blob) = blobs_by_id.get(&blob_id) {
            file_repo::decrement_blob_ref_count_by(&txn, blob_id, decrement as i32).await?;
            if i64::from(blob.ref_count) <= decrement {
                blobs_to_cleanup.push(blob.clone());
            }
        }
    }

    // 5. 配额一次性更新
    user_repo::update_storage_used(&txn, user_id, -total_size).await?;

    txn.commit().await.map_err(AsterError::from)?;

    // ── 事务后：并行物理清理，清理成功后再删 blob 元数据 ──
    let state_ref = &state;
    let cleanup_futures: Vec<_> = blobs_to_cleanup
        .iter()
        .map(|blob| async move {
            if !cleanup_unreferenced_blob(state_ref, blob).await {
                tracing::warn!(
                    blob_id = blob.id,
                    "batch purge left blob row for retry because object cleanup was incomplete"
                );
            }
        })
        .collect();

    futures::future::join_all(cleanup_futures).await;

    Ok(count)
}

/// 更新文件（重命名/移动）
pub async fn update(
    state: &AppState,
    id: i64,
    user_id: i64,
    name: Option<String>,
    folder_id: NullablePatch<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }

    // 目标文件夹校验
    let target_folder = match folder_id {
        NullablePatch::Absent => f.folder_id,
        NullablePatch::Null => None,
        NullablePatch::Value(fid) => Some(fid),
    };
    if let NullablePatch::Value(fid) = folder_id {
        let target = crate::db::repository::folder_repo::find_by_id(db, fid).await?;
        crate::utils::verify_owner(target.user_id, user_id, "folder")?;
    }

    // 文件名验证
    if let Some(ref n) = name {
        crate::utils::validate_name(n)?;
    }

    // 同名冲突检查
    let final_name = name.as_deref().unwrap_or(&f.name);
    if let Some(existing) =
        file_repo::find_by_name_in_folder(db, user_id, target_folder, final_name).await?
        && existing.id != id
    {
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in this folder",
            final_name
        )));
    }

    let mut active: file::ActiveModel = f.into();
    if let Some(n) = name {
        active.name = Set(n);
    }
    match folder_id {
        NullablePatch::Absent => {}
        NullablePatch::Null => active.folder_id = Set(None),
        NullablePatch::Value(fid) => active.folder_id = Set(Some(fid)),
    }
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

/// 移动文件到指定文件夹（None = 根目录）
///
/// 与 `update()` 的区别：`update()` 用 `NullablePatch<i64>` 区分
/// “未传字段”和“显式传 null”，而本函数的 `target_folder_id: None`
/// 明确表示“移到根目录”。
pub async fn move_file(
    state: &AppState,
    id: i64,
    user_id: i64,
    target_folder_id: Option<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }

    // 验证目标文件夹
    if let Some(fid) = target_folder_id {
        let target = crate::db::repository::folder_repo::find_by_id(db, fid).await?;
        crate::utils::verify_owner(target.user_id, user_id, "folder")?;
    }

    // 检查同名冲突
    if let Some(existing) =
        file_repo::find_by_name_in_folder(db, user_id, target_folder_id, &f.name).await?
        && existing.id != id
    {
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in target folder",
            f.name
        )));
    }

    let mut active: file::ActiveModel = f.into();
    active.folder_id = Set(target_folder_id);
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

/// 复制文件（REST API 入口，带权限检查 + 副本命名）
///
/// `dest_folder_id = None` 表示复制到根目录。
pub async fn copy_file(
    state: &AppState,
    src_id: i64,
    user_id: i64,
    dest_folder_id: Option<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let src = file_repo::find_by_id(db, src_id).await?;
    crate::utils::verify_owner(src.user_id, user_id, "file")?;

    if let Some(folder_id) = dest_folder_id {
        crate::services::folder_service::verify_folder_access(state, user_id, folder_id).await?;
    }

    // 配额检查
    let blob = file_repo::find_blob_by_id(db, src.blob_id).await?;
    user_repo::check_quota(db, user_id, blob.size).await?;

    // 副本命名：目标无冲突保留原名，有冲突则递增
    let copy_name =
        file_repo::resolve_unique_filename(db, user_id, dest_folder_id, &src.name).await?;

    duplicate_file_record(state, &src, dest_folder_id, &copy_name).await
}

/// 复制文件记录的核心逻辑（blob ref_count++ + 新文件记录 + 配额更新）
///
/// 无权限检查，供 REST copy、WebDAV COPY、recursive_copy_folder 共用。
pub async fn duplicate_file_record(
    state: &AppState,
    src: &file::Model,
    dest_folder_id: Option<i64>,
    dest_name: &str,
) -> Result<file::Model> {
    let blob = file_repo::find_blob_by_id(&state.db, src.blob_id).await?;
    let now = Utc::now();
    let blob_size = blob.size;

    // ── [事务内] 配额再校验 → blob ref_count++ → 文件记录创建 → 配额更新 ──
    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 事务内配额权威检查（防止并发 copy 绕过事务外 fast-fail）
    user_repo::check_quota(&txn, src.user_id, blob_size).await?;

    file_repo::increment_blob_ref_count(&txn, blob.id).await?;

    let new_file = file_repo::create(
        &txn,
        file::ActiveModel {
            name: Set(dest_name.to_string()),
            folder_id: Set(dest_folder_id),
            blob_id: Set(src.blob_id),
            size: Set(src.size),
            user_id: Set(src.user_id),
            mime_type: Set(src.mime_type.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    user_repo::update_storage_used(&txn, src.user_id, blob_size).await?;

    txn.commit().await.map_err(AsterError::from)?;

    Ok(new_file)
}

/// 批量复制文件记录：一次事务处理 blob ref_count + 文件创建 + 配额
///
/// 与 `duplicate_file_record` 的区别：N 个文件只开 1 次事务，
/// blob ref_count 按 blob_id 合并递增，配额只更新一次。
/// 不返回创建的 Model（递归复制场景不需要）。
pub async fn batch_duplicate_file_records(
    state: &AppState,
    src_files: &[file::Model],
    dest_folder_id: Option<i64>,
) -> Result<()> {
    if src_files.is_empty() {
        return Ok(());
    }

    let user_id = src_files[0].user_id;
    let total_size: i64 = src_files.iter().map(|f| f.size).sum();
    let now = chrono::Utc::now();

    // 事务外 fast-fail 配额检查
    user_repo::check_quota(&state.db, user_id, total_size).await?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;

    // 事务内权威配额检查
    user_repo::check_quota(&txn, user_id, total_size).await?;

    // 按 blob_id 合并 ref_count 递增
    let mut blob_counts: std::collections::HashMap<i64, i32> = std::collections::HashMap::new();
    for f in src_files {
        *blob_counts.entry(f.blob_id).or_default() += 1;
    }
    for (&blob_id, &count) in &blob_counts {
        file_repo::increment_blob_ref_count_by(&txn, blob_id, count).await?;
    }

    // 批量插入文件记录
    let models: Vec<file::ActiveModel> = src_files
        .iter()
        .map(|f| file::ActiveModel {
            name: Set(f.name.clone()),
            folder_id: Set(dest_folder_id),
            blob_id: Set(f.blob_id),
            size: Set(f.size),
            user_id: Set(f.user_id),
            mime_type: Set(f.mime_type.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .collect();
    file_repo::create_many(&txn, models).await?;

    // 配额一次性更新
    user_repo::update_storage_used(&txn, user_id, total_size).await?;

    txn.commit().await.map_err(AsterError::from)?;
    Ok(())
}

/// 覆盖文件内容（REST API 编辑入口）
///
/// 支持 ETag 乐观锁（If-Match）+ 悲观锁检查（is_locked）。
/// 自动创建版本历史。返回 (更新后的 file, 新 blob hash)。
pub async fn update_content(
    state: &AppState,
    file_id: i64,
    user_id: i64,
    body: actix_web::web::Bytes,
    if_match: Option<&str>,
) -> Result<(file::Model, String)> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, file_id).await?;
    crate::utils::verify_owner(f.user_id, user_id, "file")?;
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }

    // 悲观锁检查：如果文件被锁，只允许锁持有者或文件所有者写入
    if f.is_locked {
        let lock = crate::db::repository::lock_repo::find_by_entity(
            db,
            crate::types::EntityType::File,
            file_id,
        )
        .await?;
        if let Some(lock) = lock
            && lock.owner_id != Some(user_id)
        {
            return Err(AsterError::resource_locked(
                "file is locked by another user",
            ));
        }
    }

    // 乐观锁检查：ETag 比对
    let current_blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    if let Some(etag) = if_match {
        let expected = etag.trim_matches('"');
        if !expected.eq_ignore_ascii_case(&current_blob.hash) {
            return Err(AsterError::precondition_failed(
                "file has been modified (ETag mismatch)",
            ));
        }
    }

    // 写入临时文件
    let temp_path = format!("{}/{}", crate::utils::TEMP_DIR, uuid::Uuid::new_v4());
    tokio::fs::create_dir_all(crate::utils::TEMP_DIR)
        .await
        .map_aster_err(AsterError::storage_driver_error)?;
    tokio::fs::write(&temp_path, &body)
        .await
        .map_aster_err(AsterError::storage_driver_error)?;

    let size = body.len() as i64;

    // 复用 store_from_temp（自动版本溯源 + blob 去重）
    // skip_lock_check=true 因为上面已经手动检查过锁持有者了
    let updated = store_from_temp(
        state,
        user_id,
        f.folder_id,
        &f.name,
        &temp_path,
        size,
        Some(file_id),
        true,
    )
    .await?;

    // 获取新 blob hash
    let new_blob = file_repo::find_blob_by_id(db, updated.blob_id).await?;
    Ok((updated, new_blob.hash.clone()))
}

/// 根据优先级链解析存储策略：文件夹 → 用户默认 → 系统默认
pub async fn resolve_policy(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<crate::entities::storage_policy::Model> {
    let db = &state.db;

    // 1. 文件夹级策略
    if let Some(fid) = folder_id {
        let folder = crate::db::repository::folder_repo::find_by_id(db, fid).await?;
        if let Some(pid) = folder.policy_id {
            let cache_key = format!("policy:{pid}");
            if let Some(cached) = state
                .cache
                .get::<crate::entities::storage_policy::Model>(&cache_key)
                .await
            {
                return Ok(cached);
            }
            let policy = policy_repo::find_by_id(db, pid).await?;
            state.cache.set(&cache_key, &policy, None).await;
            return Ok(policy);
        }
    }

    // 2. 用户默认策略（缓存）
    let usp_cache_key = format!("user_default_policy:{user_id}");
    if let Some(usp) = state
        .cache
        .get::<user_storage_policy::Model>(&usp_cache_key)
        .await
    {
        let policy_cache_key = format!("policy:{}", usp.policy_id);
        if let Some(cached) = state
            .cache
            .get::<crate::entities::storage_policy::Model>(&policy_cache_key)
            .await
        {
            return Ok(cached);
        }
        let policy = policy_repo::find_by_id(db, usp.policy_id).await?;
        state.cache.set(&policy_cache_key, &policy, None).await;
        return Ok(policy);
    }

    if let Some(usp) = policy_repo::find_user_default(db, user_id).await? {
        state.cache.set(&usp_cache_key, &usp, None).await;
        let policy = policy_repo::find_by_id(db, usp.policy_id).await?;
        state
            .cache
            .set(&format!("policy:{}", policy.id), &policy, None)
            .await;
        return Ok(policy);
    }

    // 3. 系统默认策略
    policy_repo::find_default(db)
        .await?
        .ok_or_else(|| AsterError::storage_policy_not_found("no default storage policy configured"))
}

/// 直接创建空文件（0 字节），不走 multipart upload 流程。
///
/// - 校验文件名
/// - 解析存储策略
/// - blob 去重（空文件 sha256 固定）
/// - 若 blob 不存在则用 driver.put 写入 0 字节
/// - 创建文件记录并更新配额（0 字节不影响配额）
pub async fn create_empty(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
) -> Result<file::Model> {
    use crate::db::repository::{file_repo, user_repo};
    use crate::entities::file;
    use sea_orm::Set;

    let db = &state.db;

    crate::utils::validate_name(filename)?;

    // 空文件固定 sha256
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const EMPTY_SIZE: i64 = 0;

    let policy = resolve_policy(state, user_id, folder_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let storage_path = crate::utils::storage_path_from_hash(EMPTY_SHA256);

    let now = chrono::Utc::now();
    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    let txn = db.begin().await.map_err(AsterError::from)?;

    let blob =
        file_repo::find_or_create_blob(&txn, EMPTY_SHA256, EMPTY_SIZE, policy.id, &storage_path)
            .await?;
    if blob.inserted {
        driver.put(&storage_path, &[]).await?;
    }

    let final_name = file_repo::resolve_unique_filename(&txn, user_id, folder_id, filename).await?;

    let file_model = file::ActiveModel {
        name: Set(final_name),
        folder_id: Set(folder_id),
        blob_id: Set(blob.model.id),
        size: Set(EMPTY_SIZE),
        user_id: Set(user_id),
        mime_type: Set(mime),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let created = file_repo::create(&txn, file_model).await?;

    // 空文件配额为 0，仍调用以保持一致性
    user_repo::update_storage_used(&txn, user_id, EMPTY_SIZE).await?;

    txn.commit().await.map_err(AsterError::from)?;

    Ok(created)
}

// ── Lock ─────────────────────────────────────────────────────────────

/// 设置/解除文件锁，返回更新后的文件信息
pub async fn set_lock(
    state: &AppState,
    file_id: i64,
    user_id: i64,
    locked: bool,
) -> Result<file::Model> {
    use crate::services::lock_service;
    use crate::types::EntityType;

    if locked {
        lock_service::lock(state, EntityType::File, file_id, Some(user_id), None, None).await?;
    } else {
        lock_service::unlock(state, EntityType::File, file_id, user_id).await?;
    }
    get_info(state, file_id, user_id).await
}

// ── Thumbnail ────────────────────────────────────────────────────────

/// 缩略图查询结果：有数据直接返回，正在生成则标记 pending
pub struct ThumbnailResult {
    pub data: Vec<u8>,
}

/// 获取文件缩略图。返回 `Ok(Some(data))` 直接有图；`Ok(None)` 表示正在后台生成。
pub async fn get_thumbnail_data(
    state: &AppState,
    file_id: i64,
    user_id: i64,
) -> Result<Option<ThumbnailResult>> {
    let f = get_info(state, file_id, user_id).await?;
    if !thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    match thumbnail_service::get_or_enqueue(state, &blob).await? {
        Some(data) => Ok(Some(ThumbnailResult { data })),
        None => Ok(None),
    }
}
