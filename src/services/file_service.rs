use actix_multipart::Multipart;
use actix_web::HttpResponse;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{DatabaseConnection, Set};

use crate::db::repository::{file_repo, policy_repo, user_repo};
use crate::entities::{file, file_blob};
use crate::errors::{AsterError, Result};
use crate::storage::DriverRegistry;
use crate::utils::hash;

/// 上传文件
pub async fn upload(
    db: &DatabaseConnection,
    registry: &DriverRegistry,
    user_id: i64,
    payload: &mut Multipart,
    folder_id: Option<i64>,
) -> Result<file::Model> {
    // 读取 multipart field
    let mut filename = String::from("unnamed");
    let mut data = Vec::new();

    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| AsterError::file_upload_failed(e.to_string()))?;
        if let Some(cd) = field.content_disposition()
            && let Some(name) = cd.get_filename()
        {
            filename = name.to_string();
        }
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|e| AsterError::file_upload_failed(e.to_string()))?;
            data.extend_from_slice(&chunk);
        }
    }

    if data.is_empty() {
        return Err(AsterError::validation_error("empty file"));
    }

    // 确定存储策略
    let policy = resolve_policy(db, user_id, folder_id).await?;

    // 检查文件大小限制
    if policy.max_file_size > 0 && (data.len() as i64) > policy.max_file_size {
        return Err(AsterError::file_too_large(format!(
            "file size {} exceeds limit {}",
            data.len(),
            policy.max_file_size
        )));
    }

    let file_hash = hash::sha256_hex(&data);
    let size = data.len() as i64;
    let now = Utc::now();

    // 查找是否已有相同 blob
    let blob = match file_repo::find_blob_by_hash(db, &file_hash, policy.id).await? {
        Some(existing) => {
            // 增加引用计数
            let mut active: file_blob::ActiveModel = existing.clone().into();
            active.ref_count = Set(existing.ref_count + 1);
            active.updated_at = Set(now);
            use sea_orm::ActiveModelTrait;
            active.update(db).await.map_err(AsterError::from)?
        }
        None => {
            // 写入物理文件
            let storage_path = format!("{}/{}/{}", &file_hash[..2], &file_hash[2..4], &file_hash);
            let driver = registry.get_driver(&policy)?;
            driver.put(&storage_path, &data).await?;

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

    // 推测 MIME 类型
    let mime = mime_guess::from_path(&filename)
        .first_or_octet_stream()
        .to_string();

    // 检查同名文件
    if file_repo::find_by_name_in_folder(db, user_id, folder_id, &filename)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(format!(
            "file '{}' already exists in this folder",
            filename
        )));
    }

    // 创建文件记录
    let file_model = file::ActiveModel {
        name: Set(filename),
        folder_id: Set(folder_id),
        blob_id: Set(blob.id),
        user_id: Set(user_id),
        mime_type: Set(mime),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let created = file_repo::create(db, file_model).await?;

    // 更新用户已用空间
    user_repo::update_storage_used(db, user_id, size).await?;

    Ok(created)
}

/// 获取文件信息
pub async fn get_info(db: &DatabaseConnection, id: i64, user_id: i64) -> Result<file::Model> {
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }
    Ok(f)
}

/// 下载文件
pub async fn download(
    db: &DatabaseConnection,
    registry: &DriverRegistry,
    id: i64,
    user_id: i64,
) -> Result<HttpResponse> {
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    let policy = policy_repo::find_by_id(db, blob.policy_id).await?;
    let driver = registry.get_driver(&policy)?;
    let data = driver.get(&blob.storage_path).await?;

    Ok(HttpResponse::Ok()
        .content_type(f.mime_type)
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", f.name),
        ))
        .body(data))
}

/// 删除文件
pub async fn delete(
    db: &DatabaseConnection,
    registry: &DriverRegistry,
    id: i64,
    user_id: i64,
) -> Result<()> {
    let f = file_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your file"));
    }

    let blob = file_repo::find_blob_by_id(db, f.blob_id).await?;
    file_repo::delete(db, id).await?;

    // 减少引用计数，如果为 0 则删除物理文件
    if blob.ref_count <= 1 {
        let policy = policy_repo::find_by_id(db, blob.policy_id).await?;
        let driver = registry.get_driver(&policy)?;
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
    db: &DatabaseConnection,
    id: i64,
    user_id: i64,
    name: Option<String>,
    folder_id: Option<i64>,
) -> Result<file::Model> {
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
async fn resolve_policy(
    db: &DatabaseConnection,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<crate::entities::storage_policy::Model> {
    // 1. 文件夹级策略
    if let Some(fid) = folder_id {
        let folder = crate::db::repository::folder_repo::find_by_id(db, fid).await?;
        if let Some(pid) = folder.policy_id {
            return policy_repo::find_by_id(db, pid).await;
        }
    }

    // 2. 用户默认策略
    if let Some(usp) = policy_repo::find_user_default(db, user_id).await? {
        return policy_repo::find_by_id(db, usp.policy_id).await;
    }

    // 3. 系统默认策略
    policy_repo::find_default(db)
        .await?
        .ok_or_else(|| AsterError::storage_policy_not_found("no default storage policy configured"))
}
