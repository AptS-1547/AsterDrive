use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{OffsetPage, SortBy, SortOrder, load_offset_page};
use crate::db::repository::{
    file_repo, folder_repo, lock_repo, policy_repo, share_repo, upload_session_repo, user_repo,
    webdav_account_repo,
};
use crate::entities::user;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{auth_service, profile_service};
use crate::types::{UserRole, UserStatus};

/// Theme mode for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Color preset for the UI accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ColorPreset {
    #[default]
    Blue,
    Green,
    Purple,
    Orange,
}

/// File browser view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PrefViewMode {
    #[default]
    List,
    Grid,
}

/// Interface display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    En,
    Zh,
}

/// Stored user preferences (serialized as JSON in `users.config`).
/// Empty struct (all fields None) is treated as null by `get_preferences`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserPreferences {
    pub theme_mode: Option<ThemeMode>,
    pub color_preset: Option<ColorPreset>,
    pub view_mode: Option<PrefViewMode>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    pub language: Option<Language>,
}

impl UserPreferences {
    /// True if no preference fields are set (empty config).
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// PATCH request — only non-null fields are merged into existing preferences.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdatePreferencesReq {
    pub theme_mode: Option<ThemeMode>,
    pub color_preset: Option<ColorPreset>,
    pub view_mode: Option<PrefViewMode>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    pub language: Option<Language>,
}

// ── MeResponse (从 auth route 迁移) ──────────────────────────────────

/// 用户信息核心字段（不含 password_hash），用于 API 响应
#[derive(Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserCore {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: crate::types::UserRole,
    pub status: crate::types::UserStatus,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// /auth/me 响应：用户信息 + 偏好设置
#[derive(Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct MeResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: crate::types::UserRole,
    pub status: crate::types::UserStatus,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    pub access_token_expires_at: i64,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub preferences: Option<UserPreferences>,
    pub profile: profile_service::UserProfileInfo,
}

/// 通用用户响应：核心字段 + profile
#[derive(Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: crate::types::UserRole,
    pub status: crate::types::UserStatus,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub profile: profile_service::UserProfileInfo,
}

fn user_core(user: &user::Model) -> UserCore {
    UserCore {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role,
        status: user.status,
        storage_used: user.storage_used,
        storage_quota: user.storage_quota,
        policy_group_id: user.policy_group_id,
        created_at: user.created_at,
        updated_at: user.updated_at,
    }
}

pub async fn to_user_info(
    state: &AppState,
    user: &user::Model,
    audience: profile_service::AvatarAudience,
) -> Result<UserInfo> {
    let core = user_core(user);
    Ok(UserInfo {
        id: core.id,
        username: core.username,
        email: core.email,
        role: core.role,
        status: core.status,
        storage_used: core.storage_used,
        storage_quota: core.storage_quota,
        policy_group_id: core.policy_group_id,
        created_at: core.created_at,
        updated_at: core.updated_at,
        profile: profile_service::get_profile_info(state, user, audience).await?,
    })
}

pub async fn to_user_infos(
    state: &AppState,
    users: Vec<user::Model>,
    audience: profile_service::AvatarAudience,
) -> Result<Vec<UserInfo>> {
    let profile_map = profile_service::get_profile_info_map(state, &users, audience).await?;
    let gravatar_base_url = profile_service::resolve_gravatar_base_url(state);

    Ok(users
        .into_iter()
        .map(|user| UserInfo {
            id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            role: user.role,
            status: user.status,
            storage_used: user.storage_used,
            storage_quota: user.storage_quota,
            policy_group_id: user.policy_group_id,
            created_at: user.created_at,
            updated_at: user.updated_at,
            profile: profile_map.get(&user.id).cloned().unwrap_or_else(|| {
                profile_service::build_profile_info(&user, None, audience, &gravatar_base_url)
            }),
        })
        .collect())
}

/// 获取当前用户完整信息（含偏好设置）
pub async fn get_me(
    state: &AppState,
    user_id: i64,
    access_token_expires_at: i64,
) -> Result<MeResponse> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let prefs = parse_preferences(&user);
    let core = user_core(&user);
    Ok(MeResponse {
        id: core.id,
        username: core.username,
        email: core.email,
        role: core.role,
        status: core.status,
        storage_used: core.storage_used,
        storage_quota: core.storage_quota,
        policy_group_id: core.policy_group_id,
        access_token_expires_at,
        created_at: core.created_at,
        updated_at: core.updated_at,
        preferences: prefs,
        profile: profile_service::get_profile_info(
            state,
            &user,
            profile_service::AvatarAudience::SelfUser,
        )
        .await?,
    })
}

pub async fn list_all(state: &AppState) -> Result<Vec<user::Model>> {
    user_repo::find_all(&state.db).await
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
    role: Option<UserRole>,
    status: Option<UserStatus>,
) -> Result<OffsetPage<UserInfo>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        user_repo::find_paginated(&state.db, limit, offset, keyword, role, status).await
    })
    .await?;

    Ok(OffsetPage::new(
        to_user_infos(
            state,
            page.items,
            profile_service::AvatarAudience::AdminUser,
        )
        .await?,
        page.total,
        page.limit,
        page.offset,
    ))
}

pub async fn get(state: &AppState, id: i64) -> Result<UserInfo> {
    let user = user_repo::find_by_id(&state.db, id).await?;
    to_user_info(state, &user, profile_service::AvatarAudience::AdminUser).await
}

pub async fn create(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<UserInfo> {
    let user = auth_service::create_user_by_admin(state, username, email, password).await?;
    to_user_info(state, &user, profile_service::AvatarAudience::AdminUser).await
}

pub async fn update(
    state: &AppState,
    id: i64,
    role: Option<UserRole>,
    status: Option<UserStatus>,
    storage_quota: Option<i64>,
    policy_group_id: Option<i64>,
) -> Result<UserInfo> {
    if id == 1 {
        if let Some(ref status) = status
            && !status.is_active()
        {
            return Err(AsterError::validation_error(
                "cannot disable the initial admin account",
            ));
        }
        if let Some(ref role) = role
            && !role.is_admin()
        {
            return Err(AsterError::validation_error(
                "cannot demote the initial admin account",
            ));
        }
    }

    let existing = user_repo::find_by_id(&state.db, id).await?;
    let existing_policy_group_id = existing.policy_group_id;
    let role_changed = role.is_some_and(|r| r != existing.role);
    let status_changed = status.is_some_and(|s| s != existing.status);
    let policy_group_changed =
        policy_group_id.is_some_and(|group_id| existing_policy_group_id != Some(group_id));
    let current_session_version = existing.session_version;
    let mut active: user::ActiveModel = existing.into();
    if let Some(r) = role {
        active.role = Set(r);
    }
    if let Some(s) = status {
        active.status = Set(s);
    }
    if let Some(q) = storage_quota {
        active.storage_quota = Set(q);
    }
    if let Some(group_id) = policy_group_id {
        let group =
            crate::db::repository::policy_group_repo::find_group_by_id(&state.db, group_id).await?;
        if !group.is_enabled {
            return Err(AsterError::validation_error(
                "cannot assign a disabled storage policy group",
            ));
        }
        let items =
            crate::db::repository::policy_group_repo::find_group_items(&state.db, group_id).await?;
        if items.is_empty() {
            return Err(AsterError::validation_error(
                "cannot assign a storage policy group without policies",
            ));
        }
        active.policy_group_id = Set(Some(group_id));
    }
    if status_changed {
        active.session_version = Set(current_session_version.saturating_add(1));
    }
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await.map_err(AsterError::from)?;
    if policy_group_changed {
        if let Some(policy_group_id) = updated.policy_group_id {
            state
                .policy_snapshot
                .set_user_policy_group(updated.id, policy_group_id);
        } else {
            state.policy_snapshot.remove_user_policy_group(updated.id);
        }
    }
    if role_changed || status_changed {
        auth_service::invalidate_auth_snapshot_cache(state, id).await;
    }
    to_user_info(state, &updated, profile_service::AvatarAudience::AdminUser).await
}

/// 强制删除用户及其所有数据（不可逆）
///
/// 级联清理顺序：
/// 1. 永久删除所有文件（blob cleanup + 版本 + 缩略图 + 属性）
/// 2. 删除所有文件夹（+ 属性）
/// 3. 删除所有分享链接
/// 4. 删除所有 WebDAV 账号
/// 5. 删除头像上传对象
/// 6. 删除用户存储策略分配
/// 7. 清理上传 session 和临时文件
/// 8. 清理资源锁
/// 9. 删除用户记录
pub async fn force_delete(state: &AppState, target_user_id: i64) -> Result<()> {
    let db = &state.db;
    let user = user_repo::find_by_id(db, target_user_id).await?;

    // id=1 初始管理员绝对不可删除
    if target_user_id == 1 {
        return Err(AsterError::validation_error(
            "cannot delete the initial admin account",
        ));
    }

    // 其他 admin 也不可删（需要先降级为 user 再删除）
    if user.role.is_admin() {
        return Err(AsterError::validation_error(
            "cannot force-delete an admin user, demote to user first",
        ));
    }

    tracing::warn!(
        "force-deleting user #{} ({}), cascading all data",
        user.id,
        user.username
    );

    // 1. 永久删除所有文件（批量：一次事务 + 并行物理清理）
    let all_files = file_repo::find_all_by_user(db, target_user_id).await?;
    let file_count = all_files.len();
    if let Err(e) =
        crate::services::file_service::batch_purge(state, all_files, target_user_id).await
    {
        tracing::warn!("batch purge files for user #{target_user_id} failed: {e}");
    }

    // 2. 删除所有文件夹（批量属性清理 + 批量硬删除）
    let all_folders = folder_repo::find_all_by_user(db, target_user_id).await?;
    let folder_count = all_folders.len();
    let folder_ids: Vec<i64> = all_folders.iter().map(|f| f.id).collect();
    crate::db::repository::property_repo::delete_all_for_entities(
        db,
        crate::types::EntityType::Folder,
        &folder_ids,
    )
    .await?;
    folder_repo::delete_many(db, &folder_ids).await?;

    // 3. 删除所有分享链接
    share_repo::delete_all_by_user(db, target_user_id).await?;

    // 4. 删除所有 WebDAV 账号
    webdav_account_repo::delete_all_by_user(db, target_user_id).await?;

    // 5. 删除头像上传对象
    if let Err(e) = profile_service::cleanup_avatar_upload(state, target_user_id).await {
        tracing::warn!("cleanup avatar upload for user #{target_user_id} failed: {e}");
    }

    // 6. 删除用户存储策略分配
    #[allow(deprecated)]
    {
        policy_repo::delete_user_policies_by_user(db, target_user_id).await?;
    }

    // 7. 清理上传 session
    upload_session_repo::delete_all_by_user(db, target_user_id).await?;

    // 8. 清理用户持有的资源锁
    let locks = lock_repo::find_by_owner(db, target_user_id).await?;
    for lock in &locks {
        if let Err(e) = crate::services::lock_service::set_entity_locked(
            db,
            lock.entity_type,
            lock.entity_id,
            false,
        )
        .await
        {
            tracing::warn!(
                lock_id = lock.id,
                "failed to unlock during user cleanup: {e}"
            );
        }
    }
    lock_repo::delete_all_by_owner(db, target_user_id).await?;

    // 9. 删除用户记录
    user::Entity::delete_by_id(target_user_id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    state
        .policy_snapshot
        .remove_user_policy_group(target_user_id);

    tracing::info!(
        "force-deleted user #{} ({}) and all associated data ({} files, {} folders)",
        user.id,
        user.username,
        file_count,
        folder_count,
    );

    Ok(())
}

/// 从 user Model 的 config 字段解析偏好设置。
/// 空配置或解析失败返回 None，解析失败时记录日志。
pub fn parse_preferences(user: &user::Model) -> Option<UserPreferences> {
    let raw = user.config.as_deref()?;
    match serde_json::from_str::<UserPreferences>(raw) {
        Ok(prefs) if !prefs.is_empty() => Some(prefs),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("failed to parse preferences for user #{}: {e}", user.id);
            None
        }
    }
}

/// 读取用户的偏好设置（按 ID 查询后解析）。
pub async fn get_preferences(state: &AppState, user_id: i64) -> Result<Option<UserPreferences>> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    Ok(parse_preferences(&user))
}

/// 将偏好设置写入 DB。空偏好不写 DB（视为清除）。
async fn save_preferences(
    state: &AppState,
    user: user::Model,
    prefs: &UserPreferences,
) -> Result<()> {
    if prefs.is_empty() {
        return Ok(());
    }
    let json_str =
        serde_json::to_string(&prefs).map_err(|e| AsterError::internal_error(e.to_string()))?;
    let mut active = user.into_active_model();
    active.config = Set(Some(json_str));
    active.updated_at = Set(Utc::now());
    active.save(&state.db).await?;
    Ok(())
}

/// 合并更新偏好设置（只更新非 None 字段），返回完整 UserPreferences。
pub async fn update_preferences(
    state: &AppState,
    user_id: i64,
    patch: UpdatePreferencesReq,
) -> Result<UserPreferences> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;

    let mut prefs: UserPreferences = user
        .config
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // 合并更新（只覆盖非 None 的字段）
    prefs.theme_mode = patch.theme_mode.or(prefs.theme_mode);
    prefs.color_preset = patch.color_preset.or(prefs.color_preset);
    prefs.view_mode = patch.view_mode.or(prefs.view_mode);
    prefs.sort_by = patch.sort_by.or(prefs.sort_by);
    prefs.sort_order = patch.sort_order.or(prefs.sort_order);
    prefs.language = patch.language.or(prefs.language);

    save_preferences(state, user, &prefs).await?;
    Ok(prefs)
}
