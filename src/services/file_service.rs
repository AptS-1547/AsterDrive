use actix_multipart::Multipart;
use actix_web::HttpResponse;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::Set;
use tokio::io::AsyncWriteExt;

use crate::cache::CacheExt;
use crate::db::repository::{file_repo, policy_repo, user_repo};
use crate::entities::{file, file_blob, user_storage_policy};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

const HASH_BUF_SIZE: usize = 65536; // 64KB

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
pub async fn store_from_temp(
    state: &AppState,
    user_id: i64,
    folder_id: Option<i64>,
    filename: &str,
    temp_path: &str,
    size: i64,
    existing_file_id: Option<i64>,
) -> Result<file::Model> {
    let db = &state.db;

    // 流式 sha256
    let file_hash = {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;
        let mut hasher = Sha256::new();
        let mut reader = tokio::fs::File::open(temp_path)
            .await
            .map_err(|e| AsterError::file_upload_failed(format!("open temp: {e}")))?;
        let mut buf = vec![0u8; HASH_BUF_SIZE];
        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| AsterError::file_upload_failed(format!("read temp: {e}")))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        format!("{:x}", hasher.finalize())
    };

    // 策略解析
    let policy = resolve_policy(state, user_id, folder_id).await?;

    // 文件大小限制
    if policy.max_file_size > 0 && size > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            size, policy.max_file_size
        )));
    }

    // 配额检查
    let user = user_repo::find_by_id(db, user_id).await?;
    if user.storage_quota > 0 && user.storage_used + size > user.storage_quota {
        return Err(AsterError::storage_quota_exceeded(format!(
            "quota {}, used {}, need {}",
            user.storage_quota, user.storage_used, size
        )));
    }

    let now = Utc::now();
    let driver = state.driver_registry.get_driver(&policy)?;

    // Blob 去重
    let blob = match file_repo::find_blob_by_hash(db, &file_hash, policy.id).await? {
        Some(existing) => {
            let mut active: file_blob::ActiveModel = existing.clone().into();
            active.ref_count = Set(existing.ref_count + 1);
            active.updated_at = Set(now);
            use sea_orm::ActiveModelTrait;
            active.update(db).await.map_err(AsterError::from)?
        }
        None => {
            let storage_path = format!("{}/{}/{}", &file_hash[..2], &file_hash[2..4], &file_hash);
            // 零拷贝：put_file 用 rename（LocalDriver）或流式上传（S3）
            driver.put_file(&storage_path, temp_path).await?;

            file_repo::create_blob(
                db,
                file_blob::ActiveModel {
                    hash: Set(file_hash),
                    size: Set(size),
                    policy_id: Set(policy.id),
                    storage_path: Set(storage_path),
                    ref_count: Set(1),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                },
            )
            .await?
        }
    };

    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();

    if let Some(existing_id) = existing_file_id {
        // 覆盖现有文件
        let old_file = file_repo::find_by_id(db, existing_id).await?;
        let old_blob = file_repo::find_blob_by_id(db, old_file.blob_id).await?;

        let mut active: file::ActiveModel = old_file.into();
        active.blob_id = Set(blob.id);
        active.mime_type = Set(mime);
        active.updated_at = Set(now);
        use sea_orm::ActiveModelTrait;
        let updated = active.update(db).await.map_err(AsterError::from)?;

        // 旧 blob 引用计数 -1
        if old_blob.ref_count <= 1 {
            if let Err(e) =
                crate::services::thumbnail_service::delete_thumbnail(state, &old_blob).await
            {
                tracing::warn!("failed to delete thumbnail for blob {}: {e}", old_blob.id);
            }
            let old_policy = policy_repo::find_by_id(db, old_blob.policy_id).await?;
            let old_driver = state.driver_registry.get_driver(&old_policy)?;
            let _ = old_driver.delete(&old_blob.storage_path).await;
            let _ = file_repo::delete_blob(db, old_blob.id).await;
        } else {
            let mut blob_active: file_blob::ActiveModel = old_blob.clone().into();
            blob_active.ref_count = Set(old_blob.ref_count - 1);
            blob_active.updated_at = Set(now);
            use sea_orm::ActiveModelTrait;
            let _ = blob_active.update(db).await;
        }

        let delta = size - old_blob.size;
        user_repo::update_storage_used(db, user_id, delta).await?;

        Ok(updated)
    } else {
        // 新建文件
        // 检查同名文件
        if file_repo::find_by_name_in_folder(db, user_id, folder_id, filename)
            .await?
            .is_some()
        {
            return Err(AsterError::validation_error(format!(
                "file '{}' already exists in this folder",
                filename
            )));
        }

        let file_model = file::ActiveModel {
            name: Set(filename.to_string()),
            folder_id: Set(folder_id),
            blob_id: Set(blob.id),
            user_id: Set(user_id),
            mime_type: Set(mime),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        let created = file_repo::create(db, file_model).await?;
        user_repo::update_storage_used(db, user_id, size).await?;

        Ok(created)
    }
}

/// 上传文件（REST API，multipart）
pub async fn upload(
    state: &AppState,
    user_id: i64,
    payload: &mut Multipart,
    folder_id: Option<i64>,
) -> Result<file::Model> {
    // 流式写入临时文件（不在内存中缓冲整个文件）
    let mut filename = String::from("unnamed");
    let temp_path = format!("data/.tmp/{}", uuid::Uuid::new_v4());
    tokio::fs::create_dir_all("data/.tmp")
        .await
        .map_err(|e| AsterError::file_upload_failed(format!("create temp dir: {e}")))?;

    let mut temp_file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| AsterError::file_upload_failed(format!("create temp: {e}")))?;
    let mut size: i64 = 0;

    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| AsterError::file_upload_failed(e.to_string()))?;
        let is_file = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|n| n.to_string()));

        if let Some(name) = is_file {
            filename = name;
            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_err(|e| AsterError::file_upload_failed(e.to_string()))?;
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_err(|e| AsterError::file_upload_failed(format!("write temp: {e}")))?;
                size += chunk.len() as i64;
            }
        }
    }

    temp_file
        .flush()
        .await
        .map_err(|e| AsterError::file_upload_failed(format!("flush temp: {e}")))?;
    drop(temp_file);

    if size == 0 {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(AsterError::validation_error("empty file"));
    }

    let result =
        store_from_temp(state, user_id, folder_id, &filename, &temp_path, size, None).await;

    // 清理临时文件（put_file 可能已经 rename 走了，忽略错误）
    let _ = tokio::fs::remove_file(&temp_path).await;

    result
}

/// 获取文件信息
pub async fn get_info(state: &AppState, id: i64, user_id: i64) -> Result<file::Model> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{id} is in trash"
        )));
    }
    Ok(f)
}

/// 下载文件
pub async fn download(state: &AppState, id: i64, user_id: i64) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{id} is in trash"
        )));
    }

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    let policy = policy_repo::find_by_id(db, blob.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let data = driver.get(&blob.storage_path).await?;

    Ok(HttpResponse::Ok()
        .content_type(f.mime_type)
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", f.name),
        ))
        .body(data))
}

/// 下载文件（无用户校验，用于分享链接）
pub async fn download_raw(state: &AppState, id: i64) -> Result<HttpResponse> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    let policy = policy_repo::find_by_id(db, blob.policy_id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let data = driver.get(&blob.storage_path).await?;

    Ok(HttpResponse::Ok()
        .content_type(f.mime_type)
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", f.name),
        ))
        .body(data))
}

/// 删除文件（软删除 → 回收站）
pub async fn delete(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let f = file_repo::find_by_id(&state.db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }
    file_repo::soft_delete(&state.db, id).await
}

/// 永久删除文件（回收站清理用，处理 blob ref_count + 物理文件 + 缩略图 + 配额）
pub async fn purge(state: &AppState, id: i64, user_id: i64) -> Result<()> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    file_repo::delete(db, id).await?;

    // 清理属性
    crate::db::repository::property_repo::delete_all_for_entity(db, "file", id).await?;

    // 回减用户已用空间
    user_repo::update_storage_used(db, user_id, -blob.size).await?;

    // 减少引用计数，如果为 0 则删除物理文件
    if blob.ref_count <= 1 {
        if let Err(e) = crate::services::thumbnail_service::delete_thumbnail(state, &blob).await {
            tracing::warn!("failed to delete thumbnail for blob {}: {e}", blob.id);
        }
        let policy = policy_repo::find_by_id(db, blob.policy_id).await?;
        let driver = state.driver_registry.get_driver(&policy)?;
        driver.delete(&blob.storage_path).await?;
        file_repo::delete_blob(db, blob.id).await?;
    } else {
        let mut active: file_blob::ActiveModel = blob.clone().into();
        active.ref_count = Set(blob.ref_count - 1);
        active.updated_at = Set(Utc::now());
        use sea_orm::ActiveModelTrait;
        active.update(db).await.map_err(AsterError::from)?;
    }

    Ok(())
}

/// 更新文件（重命名/移动）
pub async fn update(
    state: &AppState,
    id: i64,
    user_id: i64,
    name: Option<String>,
    folder_id: Option<i64>,
) -> Result<file::Model> {
    let db = &state.db;
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }
    let mut active: file::ActiveModel = f.into();
    if let Some(n) = name {
        active.name = Set(n);
    }
    if let Some(fid) = folder_id {
        active.folder_id = Set(Some(fid));
    }
    active.updated_at = Set(Utc::now());
    use sea_orm::ActiveModelTrait;
    active.update(db).await.map_err(AsterError::from)
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
