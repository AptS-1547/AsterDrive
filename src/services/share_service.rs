use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::repository::{file_repo, folder_repo, share_repo};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{file_service, folder_service};
use crate::utils::{hash, id};

/// 公开返回给前端的分享信息（不含密码哈希和内部 ID）
#[derive(Serialize, ToSchema)]
pub struct SharePublicInfo {
    pub token: String,
    pub name: String,
    pub share_type: String, // "file" or "folder"
    pub has_password: bool,
    pub expires_at: Option<String>,
    pub is_expired: bool,
    pub download_count: i64,
    pub view_count: i64,
    pub max_downloads: i64,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
}

pub async fn create_share(
    state: &AppState,
    user_id: i64,
    file_id: Option<i64>,
    folder_id: Option<i64>,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<share::Model> {
    let db = &state.db;

    // 至少一个不为空
    if file_id.is_none() && folder_id.is_none() {
        return Err(AsterError::validation_error(
            "file_id or folder_id is required",
        ));
    }

    // 检查是否已有活跃分享
    if let Some(existing) =
        share_repo::find_active_by_resource(db, user_id, file_id, folder_id).await?
    {
        // 如果已有分享且未过期，返回错误
        let is_expired = existing.expires_at.is_some_and(|exp| exp < Utc::now());
        if !is_expired {
            return Err(AsterError::validation_error(
                "an active share already exists for this resource",
            ));
        }
        // 过期的分享自动删除，然后继续创建新的
        share_repo::delete(db, existing.id).await?;
    }

    // 校验文件/文件夹属于该用户
    if let Some(fid) = file_id {
        let f = file_repo::find_by_id(db, fid).await?;
        crate::utils::verify_owner(f.user_id, user_id, "file")?;
    }
    if let Some(fid) = folder_id {
        let f = folder_repo::find_by_id(db, fid).await?;
        crate::utils::verify_owner(f.user_id, user_id, "folder")?;
    }

    let password_hash = match password {
        Some(ref p) if !p.is_empty() => Some(hash::hash_password(p)?),
        _ => None,
    };

    let now = Utc::now();
    let model = share::ActiveModel {
        token: Set(id::new_share_token()),
        user_id: Set(user_id),
        file_id: Set(file_id),
        folder_id: Set(folder_id),
        password: Set(password_hash),
        expires_at: Set(expires_at),
        max_downloads: Set(max_downloads),
        download_count: Set(0),
        view_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    share_repo::create(db, model).await
}

pub async fn get_share_info(state: &AppState, token: &str) -> Result<SharePublicInfo> {
    let db = &state.db;
    let share = share_repo::find_by_token(db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    // increment view count (fire and forget)
    let _ = share_repo::increment_view_count(db, share.id).await;

    let (name, share_type, mime_type, size) = resolve_share_name(db, &share).await?;

    let is_expired = share.expires_at.is_some_and(|exp| exp < Utc::now());

    Ok(SharePublicInfo {
        token: share.token,
        name,
        share_type,
        has_password: share.password.is_some(),
        expires_at: share.expires_at.map(|e| e.to_rfc3339()),
        is_expired,
        download_count: share.download_count,
        view_count: share.view_count,
        max_downloads: share.max_downloads,
        mime_type,
        size,
    })
}

pub async fn verify_password(state: &AppState, token: &str, password: &str) -> Result<()> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let pw_hash = share
        .password
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("share has no password"))?;

    if !hash::verify_password(password, pw_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong share password"));
    }

    Ok(())
}

pub async fn download_shared_file(
    state: &AppState,
    token: &str,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let file_id = share
        .file_id
        .ok_or_else(|| AsterError::validation_error("this share is for a folder, not a file"))?;

    // reuse existing download logic (bypass user ownership check)
    let response = file_service::download_raw(state, file_id, if_none_match).await?;

    // only count actual downloads, not 304 cache hits
    if response.status() != actix_web::http::StatusCode::NOT_MODIFIED {
        let _ = share_repo::increment_download_count(&state.db, share.id).await;
    }

    Ok(response)
}

pub async fn download_shared_folder_file(
    state: &AppState,
    token: &str,
    file_id: i64,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let root_folder_id = share
        .folder_id
        .ok_or_else(|| AsterError::validation_error("this share is for a file, not a folder"))?;

    let file = file_repo::find_by_id(&state.db, file_id).await?;
    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }

    // 校验文件所在文件夹在分享范围内
    let file_folder_id = file
        .folder_id
        .ok_or_else(|| AsterError::auth_forbidden("file is outside shared folder scope"))?;
    verify_folder_in_share_scope(&state.db, file_folder_id, root_folder_id).await?;

    let response = file_service::download_raw(state, file_id, if_none_match).await?;

    if response.status() != actix_web::http::StatusCode::NOT_MODIFIED {
        let _ = share_repo::increment_download_count(&state.db, share.id).await;
    }

    Ok(response)
}

pub async fn list_shared_folder(
    state: &AppState,
    token: &str,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<folder_service::FolderContents> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let folder_id = share
        .folder_id
        .ok_or_else(|| AsterError::validation_error("this share is for a file, not a folder"))?;

    // list folder contents (bypass user ownership — shared access)
    folder_service::list_shared(
        state,
        folder_id,
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
        sort_by,
        sort_order,
    )
    .await
}

pub async fn list_my_shares(state: &AppState, user_id: i64) -> Result<Vec<share::Model>> {
    share_repo::find_by_user(&state.db, user_id).await
}

pub async fn delete_share(state: &AppState, share_id: i64, user_id: i64) -> Result<()> {
    let share = share_repo::find_by_id(&state.db, share_id).await?;
    crate::utils::verify_owner(share.user_id, user_id, "share")?;
    share_repo::delete(&state.db, share_id).await
}

pub async fn list_all(state: &AppState) -> Result<Vec<share::Model>> {
    share_repo::find_all(&state.db).await
}

pub async fn admin_delete_share(state: &AppState, share_id: i64) -> Result<()> {
    share_repo::find_by_id(&state.db, share_id).await?; // 校验存在
    share_repo::delete(&state.db, share_id).await
}

/// 获取公开分享文件的缩略图（公开访问，无需认证）
pub async fn get_shared_thumbnail(state: &AppState, token: &str) -> Result<Vec<u8>> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let file_id = share
        .file_id
        .ok_or_else(|| AsterError::validation_error("share is not a file"))?;

    let f = file_repo::find_by_id(&state.db, file_id).await?;
    if !crate::services::thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }

    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    crate::services::thumbnail_service::get_or_generate(state, &blob).await
}

/// 获取分享文件夹内子文件的缩略图（公开访问）
pub async fn get_shared_folder_file_thumbnail(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<Vec<u8>> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let root_folder_id = share
        .folder_id
        .ok_or_else(|| AsterError::validation_error("this share is for a file, not a folder"))?;

    let f = file_repo::find_by_id(&state.db, file_id).await?;
    if f.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }

    // 校验文件在分享范围内
    let file_folder_id = f
        .folder_id
        .ok_or_else(|| AsterError::auth_forbidden("file is outside shared folder scope"))?;
    verify_folder_in_share_scope(&state.db, file_folder_id, root_folder_id).await?;

    if !crate::services::thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }

    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    crate::services::thumbnail_service::get_or_generate(state, &blob).await
}

pub async fn list_shared_subfolder(
    state: &AppState,
    token: &str,
    folder_id: i64,
    folder_limit: u64,
    folder_offset: u64,
    file_limit: u64,
    file_cursor: Option<(String, i64)>,
    sort_by: crate::api::pagination::SortBy,
    sort_order: crate::api::pagination::SortOrder,
) -> Result<folder_service::FolderContents> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    validate_share(&share)?;

    let root_folder_id = share
        .folder_id
        .ok_or_else(|| AsterError::validation_error("this share is for a file, not a folder"))?;

    // 检查目标文件夹未删除
    let target = folder_repo::find_by_id(&state.db, folder_id).await?;
    if target.deleted_at.is_some() {
        return Err(AsterError::folder_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }

    // 校验目标文件夹在分享范围内
    verify_folder_in_share_scope(&state.db, folder_id, root_folder_id).await?;

    folder_service::list_shared(
        state,
        folder_id,
        folder_limit,
        folder_offset,
        file_limit,
        file_cursor,
        sort_by,
        sort_order,
    )
    .await
}

// ── Helpers ──────────────────────────────────────────────────────────

/// 校验 folder_id 是 root_folder_id 自身或其子孙
async fn verify_folder_in_share_scope(
    db: &DatabaseConnection,
    folder_id: i64,
    root_folder_id: i64,
) -> Result<()> {
    if folder_id == root_folder_id {
        return Ok(());
    }
    let mut current_id = folder_id;
    loop {
        let folder = folder_repo::find_by_id(db, current_id).await?;
        match folder.parent_id {
            Some(pid) if pid == root_folder_id => return Ok(()),
            Some(pid) => current_id = pid,
            None => {
                return Err(AsterError::auth_forbidden(
                    "folder is outside shared folder scope",
                ));
            }
        }
    }
}

fn validate_share(share: &share::Model) -> Result<()> {
    // 检查过期
    if let Some(exp) = share.expires_at
        && exp < Utc::now()
    {
        return Err(AsterError::share_expired("share has expired"));
    }
    // 检查下载次数限制
    if share.max_downloads > 0 && share.download_count >= share.max_downloads {
        return Err(AsterError::share_download_limit("download limit reached"));
    }
    Ok(())
}

async fn resolve_share_name(
    db: &DatabaseConnection,
    share: &share::Model,
) -> Result<(String, String, Option<String>, Option<i64>)> {
    if let Some(file_id) = share.file_id {
        let f = file_repo::find_by_id(db, file_id).await?;
        Ok((f.name, "file".to_string(), Some(f.mime_type), Some(f.size)))
    } else if let Some(folder_id) = share.folder_id {
        let f = folder_repo::find_by_id(db, folder_id).await?;
        Ok((f.name, "folder".to_string(), None, None))
    } else {
        Ok(("Unknown".to_string(), "unknown".to_string(), None, None))
    }
}
