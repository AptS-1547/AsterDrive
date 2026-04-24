use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{SortBy, SortOrder};
use crate::entities::user;
use crate::services::{auth_service, profile_service};
use crate::types::{
    BrowserOpenMode, ColorPreset, Language, PrefViewMode, ThemeMode, UserConfig,
    UserPreferences as StoredUserPreferences, UserRole, UserStatus,
};

/// API-facing user preference payload: built-in preferences plus custom frontend KV entries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserPreferences {
    pub theme_mode: Option<ThemeMode>,
    pub color_preset: Option<ColorPreset>,
    pub view_mode: Option<PrefViewMode>,
    pub browser_open_mode: Option<BrowserOpenMode>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    pub language: Option<Language>,
    pub display_time_zone: Option<String>,
    pub storage_event_stream_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, serde_json::Value>,
}

impl UserPreferences {
    pub fn is_empty(&self) -> bool {
        self.theme_mode.is_none()
            && self.color_preset.is_none()
            && self.view_mode.is_none()
            && self.browser_open_mode.is_none()
            && self.sort_by.is_none()
            && self.sort_order.is_none()
            && self.language.is_none()
            && self.display_time_zone.is_none()
            && self.storage_event_stream_enabled.is_none()
            && self.custom.is_empty()
    }
}

impl From<&UserConfig> for UserPreferences {
    fn from(config: &UserConfig) -> Self {
        let StoredUserPreferences {
            theme_mode,
            color_preset,
            view_mode,
            browser_open_mode,
            sort_by,
            sort_order,
            language,
            display_time_zone,
            storage_event_stream_enabled,
        } = config.preferences.clone();

        Self {
            theme_mode,
            color_preset,
            view_mode,
            browser_open_mode,
            sort_by,
            sort_order,
            language,
            display_time_zone,
            storage_event_stream_enabled,
            custom: config.extra.clone(),
        }
    }
}

impl From<UserConfig> for UserPreferences {
    fn from(config: UserConfig) -> Self {
        let UserConfig { preferences, extra } = config;
        let StoredUserPreferences {
            theme_mode,
            color_preset,
            view_mode,
            browser_open_mode,
            sort_by,
            sort_order,
            language,
            display_time_zone,
            storage_event_stream_enabled,
        } = preferences;

        Self {
            theme_mode,
            color_preset,
            view_mode,
            browser_open_mode,
            sort_by,
            sort_order,
            language,
            display_time_zone,
            storage_event_stream_enabled,
            custom: extra,
        }
    }
}

/// PATCH request:
/// - non-null built-in fields are merged into existing preferences
/// - `custom` entries are upserted
/// - `remove_custom_keys` entries are deleted
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdatePreferencesReq {
    pub theme_mode: Option<ThemeMode>,
    pub color_preset: Option<ColorPreset>,
    pub view_mode: Option<PrefViewMode>,
    pub browser_open_mode: Option<BrowserOpenMode>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    pub language: Option<Language>,
    pub display_time_zone: Option<String>,
    pub storage_event_stream_enabled: Option<bool>,
    #[serde(default)]
    pub custom: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub remove_custom_keys: Vec<String>,
}

/// 用户信息核心字段（不含 password_hash），用于 API 响应
#[derive(Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UserCore {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub pending_email: Option<String>,
    pub role: UserRole,
    pub status: UserStatus,
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
    pub email_verified: bool,
    pub pending_email: Option<String>,
    pub role: UserRole,
    pub status: UserStatus,
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
    pub email_verified: bool,
    pub pending_email: Option<String>,
    pub role: UserRole,
    pub status: UserStatus,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub profile: profile_service::UserProfileInfo,
}

pub(super) fn user_core(user: &user::Model) -> UserCore {
    UserCore {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        email_verified: auth_service::is_email_verified(user),
        pending_email: user.pending_email.clone(),
        role: user.role,
        status: user.status,
        storage_used: user.storage_used,
        storage_quota: user.storage_quota,
        policy_group_id: user.policy_group_id,
        created_at: user.created_at,
        updated_at: user.updated_at,
    }
}
