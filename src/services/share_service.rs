use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{
    file_repo, folder_repo, share_repo, team_repo, user_profile_repo, user_repo,
};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    batch_service, file_service, folder_service, profile_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::EntityType;
use crate::utils::{hash, id};

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ShareStatus {
    Active,
    Expired,
    Exhausted,
    Deleted,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct MyShareInfo {
    pub id: i64,
    pub token: String,
    pub resource_id: i64,
    pub resource_name: String,
    pub resource_type: EntityType,
    pub resource_deleted: bool,
    pub has_password: bool,
    pub status: ShareStatus,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub max_downloads: i64,
    pub download_count: i64,
    pub view_count: i64,
    pub remaining_downloads: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<Utc>,
}

/// 公开返回给前端的分享信息（不含密码哈希和内部 ID）
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SharePublicOwnerInfo {
    pub name: String,
    pub avatar: profile_service::AvatarInfo,
}

/// 公开返回给前端的分享信息（不含密码哈希和内部 ID）
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
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
    pub shared_by: SharePublicOwnerInfo,
}

fn validate_max_downloads(max_downloads: i64) -> Result<()> {
    if max_downloads < 0 {
        return Err(AsterError::validation_error(
            "max_downloads cannot be negative",
        ));
    }
    Ok(())
}

fn ensure_share_scope(share: &share::Model, scope: WorkspaceStorageScope) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            if share.team_id.is_some() {
                return Err(AsterError::auth_forbidden(
                    "share belongs to a team workspace",
                ));
            }
            crate::utils::verify_owner(share.user_id, user_id, "share")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if share.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden(
                    "share is outside team workspace",
                ));
            }
        }
    }

    Ok(())
}

async fn lock_share_resource_in_scope<C: sea_orm::ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    file_id: Option<i64>,
    folder_id: Option<i64>,
) -> Result<()> {
    if let Some(file_id) = file_id {
        let file = file_repo::lock_by_id(db, file_id).await?;
        workspace_storage_service::ensure_active_file_scope(&file, scope)?;
    }

    if let Some(folder_id) = folder_id {
        let folder = folder_repo::lock_by_id(db, folder_id).await?;
        workspace_storage_service::ensure_active_folder_scope(&folder, scope)?;
    }

    Ok(())
}

pub(crate) async fn create_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: Option<i64>,
    folder_id: Option<i64>,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<share::Model> {
    let db = &state.db;
    workspace_storage_service::require_scope_access(state, scope).await?;

    validate_max_downloads(max_downloads)?;

    if file_id.is_none() && folder_id.is_none() {
        return Err(AsterError::validation_error(
            "file_id or folder_id is required",
        ));
    }
    if file_id.is_some() && folder_id.is_some() {
        return Err(AsterError::validation_error(
            "only one of file_id or folder_id is allowed",
        ));
    }

    let password_hash = match password {
        Some(ref p) if !p.is_empty() => Some(hash::hash_password(p)?),
        _ => None,
    };

    let txn = db.begin().await.map_err(AsterError::from)?;
    lock_share_resource_in_scope(&txn, scope, file_id, folder_id).await?;

    let existing = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            share_repo::find_active_by_resource(&txn, user_id, file_id, folder_id).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            share_repo::find_active_by_team_resource(&txn, team_id, file_id, folder_id).await?
        }
    };

    if let Some(existing) = existing {
        let is_expired = existing.expires_at.is_some_and(|exp| exp < Utc::now());
        if !is_expired {
            return Err(AsterError::validation_error(
                "an active share already exists for this resource",
            ));
        }
        share_repo::delete(&txn, existing.id).await?;
    }

    let now = Utc::now();
    let model = share::ActiveModel {
        token: Set(id::new_share_token()),
        user_id: Set(scope.actor_user_id()),
        team_id: Set(scope.team_id()),
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
    let created = share_repo::create(&txn, model).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(created)
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
    create_share_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        file_id,
        folder_id,
        password,
        expires_at,
        max_downloads,
    )
    .await
}

pub async fn get_share_info(state: &AppState, token: &str) -> Result<SharePublicInfo> {
    let db = &state.db;
    let share = load_valid_share(state, token).await?;

    // increment view count (fire and forget)
    if let Err(e) = share_repo::increment_view_count(db, share.id).await {
        tracing::warn!(share_id = share.id, "failed to increment view count: {e}");
    }

    let (name, share_type, mime_type, size) = resolve_share_name(db, &share).await?;
    let shared_by = resolve_share_owner_info(state, &share).await?;

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
        shared_by,
    })
}

fn resolve_share_owner_name(
    user: &crate::entities::user::Model,
    profile: Option<&crate::entities::user_profile::Model>,
) -> String {
    profile
        .and_then(|p| p.display_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| user.username.clone())
}

async fn resolve_share_owner_info(
    state: &AppState,
    share: &share::Model,
) -> Result<SharePublicOwnerInfo> {
    let user = user_repo::find_by_id(&state.db, share.user_id).await?;
    let profile = user_profile_repo::find_by_user_id(&state.db, share.user_id).await?;
    let gravatar_base_url = profile_service::resolve_gravatar_base_url(state);

    Ok(SharePublicOwnerInfo {
        name: resolve_share_owner_name(&user, profile.as_ref()),
        avatar: profile_service::build_share_public_avatar_info(
            &user,
            profile.as_ref(),
            &share.token,
            &gravatar_base_url,
        ),
    })
}

pub async fn get_share_avatar_bytes(state: &AppState, token: &str, size: u32) -> Result<Vec<u8>> {
    let share = load_valid_share(state, token).await?;
    profile_service::get_avatar_bytes(state, share.user_id, size).await
}

pub async fn verify_password(state: &AppState, token: &str, password: &str) -> Result<()> {
    let share = load_valid_share(state, token).await?;

    let pw_hash = share
        .password
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("share has no password"))?;

    if !hash::verify_password(password, pw_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong share password"));
    }

    Ok(())
}

// ── Cookie 签名（密码验证后标记） ─────────────────────────────────────

/// SHA256 签名：防止伪造分享密码验证 cookie
pub fn sign_share_cookie(token: &str, secret: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(format!("share_verified:{secret}:{token}").as_bytes());
    crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
}

/// 验证分享密码 cookie 签名（常量时间比较）
pub fn verify_share_cookie(token: &str, cookie_value: &str, secret: &str) -> bool {
    let expected = sign_share_cookie(token, secret);
    if expected.len() != cookie_value.len() {
        return false;
    }
    expected
        .bytes()
        .zip(cookie_value.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// 如果分享有密码，校验 cookie 签名是否有效。
/// `cookie_value` 由路由从 `HttpRequest` 提取传入（不依赖 HTTP 类型）。
pub async fn check_share_password_cookie(
    state: &AppState,
    token: &str,
    cookie_value: Option<&str>,
) -> Result<()> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    if share.password.is_some() {
        let value = cookie_value
            .ok_or_else(|| AsterError::share_password_required("password verification required"))?;

        if !verify_share_cookie(token, value, &state.config.auth.jwt_secret) {
            return Err(AsterError::share_password_required(
                "invalid verification cookie",
            ));
        }
    }
    Ok(())
}

/// 验证密码 + 生成 cookie 签名（verify_password handler 用）
pub struct PasswordVerified {
    pub cookie_signature: String,
}

pub async fn verify_password_and_sign(
    state: &AppState,
    token: &str,
    password: &str,
) -> Result<PasswordVerified> {
    verify_password(state, token, password).await?;
    Ok(PasswordVerified {
        cookie_signature: sign_share_cookie(token, &state.config.auth.jwt_secret),
    })
}

pub async fn download_shared_file(
    state: &AppState,
    token: &str,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let share = load_valid_share(state, token).await?;
    let file = load_share_file_resource(state, &share).await?;
    download_share_resource_with_disposition(
        state,
        &share,
        &file,
        file_service::DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

pub async fn download_shared_folder_file(
    state: &AppState,
    token: &str,
    file_id: i64,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let (share, file) = load_shared_folder_file_target(state, token, file_id).await?;
    download_share_resource_with_disposition(
        state,
        &share,
        &file,
        file_service::DownloadDisposition::Attachment,
        if_none_match,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
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
    let (_, folder_id) = load_valid_folder_share_root(state, token).await?;

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

async fn list_shares_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
) -> Result<Vec<MyShareInfo>> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let shares = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            share_repo::find_by_user(&state.db, user_id).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            share_repo::find_by_team(&state.db, team_id).await?
        }
    };
    build_my_share_infos(&state.db, shares).await
}

pub(crate) async fn list_shares_paginated_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (shares, total) = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                share_repo::find_by_user_paginated(&state.db, user_id, limit, offset).await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                share_repo::find_by_team_paginated(&state.db, team_id, limit, offset).await?
            }
        };
        let items = build_my_share_infos(&state.db, shares).await?;
        Ok((items, total))
    })
    .await
}

async fn load_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
) -> Result<share::Model> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let share = share_repo::find_by_id(&state.db, share_id).await?;
    ensure_share_scope(&share, scope)?;
    Ok(share)
}

pub(crate) async fn delete_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
) -> Result<()> {
    load_share_in_scope(state, scope, share_id).await?;
    share_repo::delete(&state.db, share_id).await
}

pub(crate) async fn update_share_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<share::Model> {
    validate_max_downloads(max_downloads)?;

    let existing = load_share_in_scope(state, scope, share_id).await?;
    let mut active: share::ActiveModel = existing.into();

    if let Some(password) = password {
        active.password = if password.is_empty() {
            Set(None)
        } else {
            Set(Some(hash::hash_password(&password)?))
        };
    }

    active.expires_at = Set(expires_at);
    active.max_downloads = Set(max_downloads);
    active.updated_at = Set(Utc::now());

    share_repo::update(&state.db, active).await
}

pub(crate) async fn batch_delete_shares_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    let mut result = batch_service::BatchResult {
        succeeded: 0,
        failed: 0,
        errors: vec![],
    };

    for &id in share_ids {
        match delete_share_in_scope(state, scope, id).await {
            Ok(()) => result.succeeded += 1,
            Err(e) => {
                result.failed += 1;
                result.errors.push(batch_service::BatchItemError {
                    entity_type: "share".to_string(),
                    entity_id: id,
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(result)
}

pub async fn list_my_shares(state: &AppState, user_id: i64) -> Result<Vec<MyShareInfo>> {
    list_shares_in_scope(state, WorkspaceStorageScope::Personal { user_id }).await
}

pub async fn list_team_shares(
    state: &AppState,
    team_id: i64,
    user_id: i64,
) -> Result<Vec<MyShareInfo>> {
    list_shares_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
    )
    .await
}

pub async fn list_my_shares_paginated(
    state: &AppState,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    list_shares_paginated_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        limit,
        offset,
    )
    .await
}

pub async fn list_team_shares_paginated(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<MyShareInfo>> {
    list_shares_paginated_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        limit,
        offset,
    )
    .await
}

pub async fn delete_share(state: &AppState, share_id: i64, user_id: i64) -> Result<()> {
    delete_share_in_scope(state, WorkspaceStorageScope::Personal { user_id }, share_id).await
}

pub async fn delete_team_share(
    state: &AppState,
    team_id: i64,
    share_id: i64,
    user_id: i64,
) -> Result<()> {
    delete_share_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_id,
    )
    .await
}

pub async fn update_share(
    state: &AppState,
    share_id: i64,
    user_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<share::Model> {
    update_share_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        share_id,
        password,
        expires_at,
        max_downloads,
    )
    .await
}

pub async fn update_team_share(
    state: &AppState,
    team_id: i64,
    share_id: i64,
    user_id: i64,
    password: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
    max_downloads: i64,
) -> Result<share::Model> {
    update_share_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_id,
        password,
        expires_at,
        max_downloads,
    )
    .await
}

pub fn validate_batch_share_ids(share_ids: &[i64]) -> Result<()> {
    if share_ids.is_empty() {
        return Err(AsterError::validation_error(
            "at least one share ID is required",
        ));
    }
    if share_ids.len() > batch_service::MAX_BATCH_ITEMS {
        return Err(AsterError::validation_error(format!(
            "batch size cannot exceed {} items",
            batch_service::MAX_BATCH_ITEMS
        )));
    }
    Ok(())
}

pub async fn batch_delete_shares(
    state: &AppState,
    user_id: i64,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    validate_batch_share_ids(share_ids)?;
    batch_delete_shares_in_scope(
        state,
        WorkspaceStorageScope::Personal { user_id },
        share_ids,
    )
    .await
}

pub async fn batch_delete_team_shares(
    state: &AppState,
    team_id: i64,
    user_id: i64,
    share_ids: &[i64],
) -> Result<batch_service::BatchResult> {
    validate_batch_share_ids(share_ids)?;
    batch_delete_shares_in_scope(
        state,
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: user_id,
        },
        share_ids,
    )
    .await
}

pub async fn list_all(state: &AppState) -> Result<Vec<share::Model>> {
    share_repo::find_all(&state.db).await
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<share::Model>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        share_repo::find_paginated(&state.db, limit, offset).await
    })
    .await
}

pub async fn admin_delete_share(state: &AppState, share_id: i64) -> Result<()> {
    share_repo::find_by_id(&state.db, share_id).await?; // 校验存在
    share_repo::delete(&state.db, share_id).await
}

async fn build_my_share_infos(
    db: &DatabaseConnection,
    shares: Vec<share::Model>,
) -> Result<Vec<MyShareInfo>> {
    let file_ids: Vec<i64> = shares.iter().filter_map(|share| share.file_id).collect();
    let folder_ids: Vec<i64> = shares.iter().filter_map(|share| share.folder_id).collect();

    let files = file_repo::find_by_ids(db, &file_ids).await?;
    let folders = folder_repo::find_by_ids(db, &folder_ids).await?;

    let file_map: HashMap<i64, crate::entities::file::Model> =
        files.into_iter().map(|file| (file.id, file)).collect();
    let folder_map: HashMap<i64, crate::entities::folder::Model> = folders
        .into_iter()
        .map(|folder| (folder.id, folder))
        .collect();

    let mut items = Vec::with_capacity(shares.len());
    for share in shares {
        let (resource_id, resource_name, resource_type, resource_deleted) =
            resolve_share_resource(&share, &file_map, &folder_map);
        let status = resolve_share_status(&share, resource_deleted);
        let remaining_downloads = remaining_downloads(share.max_downloads, share.download_count);

        items.push(MyShareInfo {
            id: share.id,
            token: share.token,
            resource_id,
            resource_name,
            resource_type,
            resource_deleted,
            has_password: share.password.is_some(),
            status,
            expires_at: share.expires_at,
            max_downloads: share.max_downloads,
            download_count: share.download_count,
            view_count: share.view_count,
            remaining_downloads,
            created_at: share.created_at,
            updated_at: share.updated_at,
        });
    }

    Ok(items)
}

/// 获取公开分享文件的缩略图（公开访问，无需认证）
pub async fn get_shared_thumbnail(state: &AppState, token: &str) -> Result<Vec<u8>> {
    let share = load_valid_share(state, token).await?;
    let f = load_share_file_resource(state, &share).await?;
    crate::services::thumbnail_service::ensure_supported_mime(&f.mime_type)?;

    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    crate::services::thumbnail_service::get_or_generate(state, &blob).await
}

/// 获取分享文件夹内子文件的缩略图（公开访问）
pub async fn get_shared_folder_file_thumbnail(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<Vec<u8>> {
    let (_, f) = load_shared_folder_file_target(state, token, file_id).await?;

    crate::services::thumbnail_service::ensure_supported_mime(&f.mime_type)?;

    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    crate::services::thumbnail_service::get_or_generate(state, &blob).await
}

pub(crate) async fn load_preview_shared_file(
    state: &AppState,
    token: &str,
) -> Result<(share::Model, crate::entities::file::Model)> {
    let share = load_valid_share(state, token).await?;
    let file = load_share_file_resource(state, &share).await?;
    Ok((share, file))
}

pub(crate) async fn load_preview_shared_folder_file(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<(share::Model, crate::entities::file::Model)> {
    load_shared_folder_file_target(state, token, file_id).await
}

#[allow(clippy::too_many_arguments)]
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
    let (_, target) = load_shared_subfolder_target(state, token, folder_id).await?;

    folder_service::list_shared(
        state,
        target.id,
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

async fn load_valid_share(state: &AppState, token: &str) -> Result<share::Model> {
    let share = load_share_record(state, token).await?;
    validate_share(&share)?;
    Ok(share)
}

async fn load_share_record(state: &AppState, token: &str) -> Result<share::Model> {
    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;
    if let Some(team_id) = share.team_id {
        match team_repo::find_active_by_id(&state.db, team_id).await {
            Ok(_) => {}
            Err(AsterError::RecordNotFound(_)) => {
                return Err(AsterError::share_not_found(format!("token={token}")));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(share)
}

fn ensure_share_matches_file(
    share: &share::Model,
    file: &crate::entities::file::Model,
) -> Result<()> {
    if let Some(team_id) = share.team_id {
        if file.team_id != Some(team_id) {
            return Err(AsterError::auth_forbidden("file is outside shared scope"));
        }
    } else {
        file_service::ensure_personal_file_scope(file)?;
        crate::utils::verify_owner(file.user_id, share.user_id, "file")?;
    }
    Ok(())
}

fn ensure_share_matches_folder(
    share: &share::Model,
    folder: &crate::entities::folder::Model,
) -> Result<()> {
    if let Some(team_id) = share.team_id {
        if folder.team_id != Some(team_id) {
            return Err(AsterError::auth_forbidden("folder is outside shared scope"));
        }
    } else {
        folder_service::ensure_personal_folder_scope(folder)?;
        crate::utils::verify_owner(folder.user_id, share.user_id, "folder")?;
    }
    Ok(())
}

async fn load_share_file_resource(
    state: &AppState,
    share: &share::Model,
) -> Result<crate::entities::file::Model> {
    let file_id = share
        .file_id
        .ok_or_else(|| AsterError::validation_error("this share is for a folder, not a file"))?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_share_matches_file(share, &file)?;
    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }
    Ok(file)
}

async fn load_share_folder_resource(
    state: &AppState,
    share: &share::Model,
) -> Result<crate::entities::folder::Model> {
    let folder_id = share
        .folder_id
        .ok_or_else(|| AsterError::validation_error("this share is for a file, not a folder"))?;
    let folder = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_share_matches_folder(share, &folder)?;
    if folder.deleted_at.is_some() {
        return Err(AsterError::folder_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    Ok(folder)
}

async fn load_valid_folder_share_root(
    state: &AppState,
    token: &str,
) -> Result<(share::Model, i64)> {
    let share = load_valid_share(state, token).await?;
    let root = load_share_folder_resource(state, &share).await?;
    Ok((share, root.id))
}

async fn load_shared_folder_file_target(
    state: &AppState,
    token: &str,
    file_id: i64,
) -> Result<(share::Model, crate::entities::file::Model)> {
    let (share, root_folder_id) = load_valid_folder_share_root(state, token).await?;
    let file = file_repo::find_by_id(&state.db, file_id).await?;
    ensure_share_matches_file(&share, &file)?;
    if file.deleted_at.is_some() {
        return Err(AsterError::file_not_found(format!(
            "file #{file_id} is in trash"
        )));
    }
    let file_folder_id = file
        .folder_id
        .ok_or_else(|| AsterError::auth_forbidden("file is outside shared folder scope"))?;
    folder_service::verify_folder_in_scope(&state.db, file_folder_id, root_folder_id).await?;
    Ok((share, file))
}

async fn load_shared_subfolder_target(
    state: &AppState,
    token: &str,
    folder_id: i64,
) -> Result<(share::Model, crate::entities::folder::Model)> {
    let (share, root_folder_id) = load_valid_folder_share_root(state, token).await?;
    let target = folder_repo::find_by_id(&state.db, folder_id).await?;
    ensure_share_matches_folder(&share, &target)?;
    if target.deleted_at.is_some() {
        return Err(AsterError::folder_not_found(format!(
            "folder #{folder_id} is in trash"
        )));
    }
    folder_service::verify_folder_in_scope(&state.db, folder_id, root_folder_id).await?;
    Ok((share, target))
}

async fn download_share_resource_with_disposition(
    state: &AppState,
    share: &share::Model,
    file: &crate::entities::file::Model,
    disposition: file_service::DownloadDisposition,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;

    if let Some(if_none_match) = if_none_match
        && file_service::if_none_match_matches(if_none_match, &blob.hash)
    {
        return file_service::build_stream_response_with_disposition(
            state,
            file,
            &blob,
            disposition,
            Some(if_none_match),
        )
        .await;
    }

    match share_repo::increment_download_count(&state.db, share.id).await {
        Ok(true) => {}
        Ok(false) => {
            return Err(AsterError::share_download_limit("download limit reached"));
        }
        Err(e) => {
            tracing::warn!(
                share_id = share.id,
                "failed to increment download count: {e}"
            );
            return Err(e);
        }
    }

    match file_service::build_stream_response_with_disposition(
        state,
        file,
        &blob,
        disposition,
        None,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            match share_repo::decrement_download_count(&state.db, share.id).await {
                Ok(true) => {}
                Ok(false) => {
                    tracing::warn!(
                        share_id = share.id,
                        "failed to roll back download count after response build failure"
                    );
                }
                Err(rollback_error) => {
                    tracing::warn!(
                        share_id = share.id,
                        "failed to roll back download count after response build failure: {rollback_error}"
                    );
                }
            }
            Err(error)
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
    validate_share_download_limit(share)?;
    Ok(())
}

fn validate_share_download_limit(share: &share::Model) -> Result<()> {
    if share.max_downloads > 0 && share.download_count >= share.max_downloads {
        return Err(AsterError::share_download_limit("download limit reached"));
    }

    Ok(())
}

fn resolve_share_resource(
    share: &share::Model,
    file_map: &HashMap<i64, crate::entities::file::Model>,
    folder_map: &HashMap<i64, crate::entities::folder::Model>,
) -> (i64, String, EntityType, bool) {
    if let Some(file_id) = share.file_id {
        if let Some(file) = file_map.get(&file_id) {
            return (
                file_id,
                file.name.clone(),
                EntityType::File,
                file.deleted_at.is_some(),
            );
        }
        return (file_id, "Unknown file".to_string(), EntityType::File, true);
    }

    if let Some(folder_id) = share.folder_id {
        if let Some(folder) = folder_map.get(&folder_id) {
            return (
                folder_id,
                folder.name.clone(),
                EntityType::Folder,
                folder.deleted_at.is_some(),
            );
        }
        return (
            folder_id,
            "Unknown folder".to_string(),
            EntityType::Folder,
            true,
        );
    }

    (0, "Unknown resource".to_string(), EntityType::File, true)
}

fn resolve_share_status(share: &share::Model, resource_deleted: bool) -> ShareStatus {
    if resource_deleted {
        return ShareStatus::Deleted;
    }
    if share
        .expires_at
        .is_some_and(|expires_at| expires_at < Utc::now())
    {
        return ShareStatus::Expired;
    }
    if share.max_downloads > 0 && share.download_count >= share.max_downloads {
        return ShareStatus::Exhausted;
    }
    ShareStatus::Active
}

fn remaining_downloads(max_downloads: i64, download_count: i64) -> Option<i64> {
    (max_downloads > 0).then_some((max_downloads - download_count).max(0))
}

async fn resolve_share_name(
    db: &DatabaseConnection,
    share: &share::Model,
) -> Result<(String, String, Option<String>, Option<i64>)> {
    if let Some(file_id) = share.file_id {
        let f = file_repo::find_by_id(db, file_id).await?;
        ensure_share_matches_file(share, &f)?;
        if f.deleted_at.is_some() {
            return Err(AsterError::file_not_found(format!(
                "file #{file_id} is in trash"
            )));
        }
        Ok((f.name, "file".to_string(), Some(f.mime_type), Some(f.size)))
    } else if let Some(folder_id) = share.folder_id {
        let f = folder_repo::find_by_id(db, folder_id).await?;
        ensure_share_matches_folder(share, &f)?;
        if f.deleted_at.is_some() {
            return Err(AsterError::folder_not_found(format!(
                "folder #{folder_id} is in trash"
            )));
        }
        Ok((f.name, "folder".to_string(), None, None))
    } else {
        Ok(("Unknown".to_string(), "unknown".to_string(), None, None))
    }
}
