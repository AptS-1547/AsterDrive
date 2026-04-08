use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::config::auth_runtime;
use crate::config::branding;
use crate::config::cors;
use crate::config::definitions::ALL_CONFIGS;
use crate::db::repository::config_repo;
use crate::entities::system_config;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::audit_service::{self, AuditContext};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub async fn list_all(state: &AppState) -> Result<Vec<system_config::Model>> {
    config_repo::find_all(&state.db).await
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<system_config::Model>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        config_repo::find_paginated(&state.db, limit, offset).await
    })
    .await
}

pub async fn get_by_key(state: &AppState, key: &str) -> Result<system_config::Model> {
    config_repo::find_by_key(&state.db, key)
        .await?
        .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))
}

pub async fn set(
    state: &AppState,
    key: &str,
    value: &str,
    updated_by: i64,
) -> Result<system_config::Model> {
    let mut normalized_value = value.to_string();

    // 系统配置做值类型校验
    if let Some(def) = ALL_CONFIGS.iter().find(|d| d.key == key) {
        validate_value_type(def.value_type, value)?;
        normalized_value = normalize_system_value(state, key, value)?;
    }

    let config = config_repo::upsert(&state.db, key, &normalized_value, updated_by).await?;
    state.runtime_config.apply(config.clone());
    Ok(config)
}

pub async fn delete(state: &AppState, key: &str) -> Result<()> {
    config_repo::delete_by_key(&state.db, key).await?;
    state.runtime_config.remove(key);
    Ok(())
}

pub async fn set_with_audit(
    state: &AppState,
    key: &str,
    value: &str,
    updated_by: i64,
    audit_ctx: &AuditContext,
) -> Result<system_config::Model> {
    let config = set(state, key, value, updated_by).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::ConfigUpdate,
        None,
        None,
        Some(key),
        audit_service::details(audit_service::ConfigUpdateDetails { value }),
    )
    .await;
    Ok(config)
}

/// 校验值是否匹配声明的类型
fn validate_value_type(value_type: &str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    match value_type {
        "boolean" => {
            if trimmed != "true" && trimmed != "false" {
                return Err(AsterError::validation_error(
                    "boolean config must be 'true' or 'false'",
                ));
            }
        }
        "number" => {
            if trimmed.parse::<f64>().is_err() {
                return Err(AsterError::validation_error(
                    "number config must be a valid number",
                ));
            }
        }
        _ => {} // string 不做校验
    }
    Ok(())
}

fn normalize_system_value(state: &AppState, key: &str, value: &str) -> Result<String> {
    match key {
        auth_runtime::AUTH_COOKIE_SECURE_KEY => {
            auth_runtime::normalize_cookie_secure_config_value(value)
        }
        auth_runtime::AUTH_ACCESS_TOKEN_TTL_SECS_KEY
        | auth_runtime::AUTH_REFRESH_TOKEN_TTL_SECS_KEY => {
            auth_runtime::normalize_token_ttl_config_value(key, value)
        }
        cors::CORS_ALLOWED_ORIGINS_KEY => {
            let normalized = cors::normalize_allowed_origins_config_value(value)?;
            let parsed = cors::parse_allowed_origins_value(&normalized)?;
            let allow_credentials = state
                .runtime_config
                .get_bool(cors::CORS_ALLOW_CREDENTIALS_KEY)
                .unwrap_or(cors::DEFAULT_CORS_ALLOW_CREDENTIALS);
            cors::validate_runtime_cors_combination(&parsed, allow_credentials)?;
            Ok(normalized)
        }
        cors::CORS_ALLOW_CREDENTIALS_KEY => {
            let normalized = cors::normalize_allow_credentials_config_value(value)?;
            let allow_credentials = normalized == "true";
            let current_origins = state
                .runtime_config
                .get(cors::CORS_ALLOWED_ORIGINS_KEY)
                .unwrap_or_default();
            let parsed = cors::parse_allowed_origins_value(&current_origins)?;
            cors::validate_runtime_cors_combination(&parsed, allow_credentials)?;
            Ok(normalized)
        }
        cors::CORS_MAX_AGE_SECS_KEY => cors::normalize_max_age_config_value(value),
        branding::BRANDING_TITLE_KEY => branding::normalize_title_config_value(value),
        branding::BRANDING_DESCRIPTION_KEY => branding::normalize_description_config_value(value),
        branding::BRANDING_FAVICON_URL_KEY => branding::normalize_favicon_url_config_value(value),
        _ => Ok(value.to_string()),
    }
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicBranding {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
}

pub fn get_public_branding(state: &AppState) -> PublicBranding {
    PublicBranding {
        title: branding::title_or_default(&state.runtime_config),
        description: branding::description_or_default(&state.runtime_config),
        favicon_url: branding::favicon_url_or_default(&state.runtime_config),
    }
}

// ── Config Schema ─────────────────────────────────────────────────────

/// 系统配置的 schema 信息（从 ALL_CONFIGS 生成）
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ConfigSchemaItem {
    pub key: String,
    pub label_i18n_key: String,
    pub description_i18n_key: String,
    pub value_type: String,
    pub default_value: String,
    pub category: String,
    pub description: String,
    pub requires_restart: bool,
    pub is_sensitive: bool,
}

/// 返回所有系统配置的 schema 信息
pub fn get_schema() -> Vec<ConfigSchemaItem> {
    ALL_CONFIGS
        .iter()
        .map(|def| ConfigSchemaItem {
            key: def.key.to_string(),
            label_i18n_key: def.label_i18n_key.to_string(),
            description_i18n_key: def.description_i18n_key.to_string(),
            value_type: def.value_type.to_string(),
            default_value: (def.default_fn)(),
            category: def.category.to_string(),
            description: def.description.to_string(),
            requires_restart: def.requires_restart,
            is_sensitive: def.is_sensitive,
        })
        .collect()
}
