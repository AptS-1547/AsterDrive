use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::config::auth_runtime;
use crate::config::avatar;
use crate::config::branding;
use crate::config::cors;
use crate::config::definitions::ALL_CONFIGS;
use crate::config::mail;
use crate::config::site_url;
use crate::db::repository::{config_repo, user_repo};
use crate::entities::system_config;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    mail_service,
};
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub const MAIL_CONFIG_ACTION_KEY: &str = "mail";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub enum ConfigActionType {
    SendTestEmail,
}

impl ConfigActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SendTestEmail => "send_test_email",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigActionResult {
    pub message: String,
    pub target_email: Option<String>,
}

pub async fn list_all(state: &AppState) -> Result<Vec<system_config::Model>> {
    Ok(config_repo::find_all(&state.db)
        .await?
        .into_iter()
        .map(apply_system_config_definition)
        .collect())
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<system_config::Model>> {
    let mut page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        config_repo::find_paginated(&state.db, limit, offset).await
    })
    .await?;
    page.items = page
        .items
        .into_iter()
        .map(apply_system_config_definition)
        .collect();
    Ok(page)
}

pub async fn get_by_key(state: &AppState, key: &str) -> Result<system_config::Model> {
    config_repo::find_by_key(&state.db, key)
        .await?
        .map(apply_system_config_definition)
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

    let config = apply_system_config_definition(
        config_repo::upsert(&state.db, key, &normalized_value, updated_by).await?,
    );
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

pub async fn execute_action(
    state: &AppState,
    key: &str,
    action: ConfigActionType,
    actor_user_id: i64,
    target_email: Option<&str>,
) -> Result<ConfigActionResult> {
    match key {
        MAIL_CONFIG_ACTION_KEY => {
            execute_mail_action(state, action, actor_user_id, target_email).await
        }
        _ => Err(AsterError::record_not_found(format!(
            "config action target '{key}'"
        ))),
    }
}

async fn execute_mail_action(
    state: &AppState,
    action: ConfigActionType,
    actor_user_id: i64,
    target_email: Option<&str>,
) -> Result<ConfigActionResult> {
    match action {
        ConfigActionType::SendTestEmail => {
            let actor = user_repo::find_by_id(&state.db, actor_user_id).await?;
            let requested_target = target_email.unwrap_or(&actor.email);
            let normalized_target = mail::normalize_mail_address_config_value(requested_target)?;
            if normalized_target.is_empty() {
                return Err(AsterError::validation_error("target_email is required"));
            }

            tracing::debug!(
                actor_user_id,
                actor_username = %actor.username,
                target_email = %normalized_target,
                action = %action.as_str(),
                "config: executing mail action"
            );

            mail_service::send_test_email(state, &normalized_target, Some(&actor.username)).await?;

            Ok(ConfigActionResult {
                message: format!("Test email sent to {normalized_target}"),
                target_email: Some(normalized_target),
            })
        }
    }
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
        "string" | "multiline" => {}
        _ => {} // 其他类型按 string 处理
    }
    Ok(())
}

fn normalize_system_value(state: &AppState, key: &str, value: &str) -> Result<String> {
    match key {
        avatar::AVATAR_DIR_KEY => avatar::normalize_avatar_dir_config_value(value),
        auth_runtime::AUTH_COOKIE_SECURE_KEY => {
            auth_runtime::normalize_cookie_secure_config_value(value)
        }
        auth_runtime::AUTH_ALLOW_USER_REGISTRATION_KEY => {
            auth_runtime::normalize_allow_user_registration_config_value(value)
        }
        auth_runtime::AUTH_ACCESS_TOKEN_TTL_SECS_KEY
        | auth_runtime::AUTH_REFRESH_TOKEN_TTL_SECS_KEY
        | auth_runtime::AUTH_REGISTER_ACTIVATION_TTL_SECS_KEY
        | auth_runtime::AUTH_CONTACT_CHANGE_TTL_SECS_KEY
        | auth_runtime::AUTH_PASSWORD_RESET_TTL_SECS_KEY
        | auth_runtime::AUTH_PASSWORD_RESET_REQUEST_COOLDOWN_SECS_KEY
        | auth_runtime::AUTH_CONTACT_VERIFICATION_RESEND_COOLDOWN_SECS_KEY => {
            auth_runtime::normalize_token_ttl_config_value(key, value)
        }
        cors::CORS_ENABLED_KEY => cors::normalize_enabled_config_value(value),
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
        mail::MAIL_SMTP_HOST_KEY => mail::normalize_smtp_host_config_value(value),
        mail::MAIL_SMTP_PORT_KEY => mail::normalize_smtp_port_config_value(value),
        mail::MAIL_FROM_ADDRESS_KEY => mail::normalize_mail_address_config_value(value),
        mail::MAIL_FROM_NAME_KEY => mail::normalize_mail_name_config_value(value),
        mail::MAIL_SECURITY_KEY => mail::normalize_mail_security_config_value(value),
        mail::MAIL_TEMPLATE_REGISTER_ACTIVATION_SUBJECT_KEY
        | mail::MAIL_TEMPLATE_CONTACT_CHANGE_CONFIRMATION_SUBJECT_KEY
        | mail::MAIL_TEMPLATE_PASSWORD_RESET_SUBJECT_KEY
        | mail::MAIL_TEMPLATE_PASSWORD_RESET_NOTICE_SUBJECT_KEY
        | mail::MAIL_TEMPLATE_CONTACT_CHANGE_NOTICE_SUBJECT_KEY => {
            mail::normalize_mail_template_subject_config_value(key, value)
        }
        mail::MAIL_TEMPLATE_REGISTER_ACTIVATION_HTML_KEY
        | mail::MAIL_TEMPLATE_CONTACT_CHANGE_CONFIRMATION_HTML_KEY
        | mail::MAIL_TEMPLATE_PASSWORD_RESET_HTML_KEY
        | mail::MAIL_TEMPLATE_PASSWORD_RESET_NOTICE_HTML_KEY
        | mail::MAIL_TEMPLATE_CONTACT_CHANGE_NOTICE_HTML_KEY => {
            mail::normalize_mail_template_body_config_value(key, value)
        }
        site_url::PUBLIC_SITE_URL_KEY => site_url::normalize_public_site_url_config_value(value),
        branding::BRANDING_TITLE_KEY => branding::normalize_title_config_value(value),
        branding::BRANDING_DESCRIPTION_KEY => branding::normalize_description_config_value(value),
        branding::BRANDING_FAVICON_URL_KEY => branding::normalize_favicon_url_config_value(value),
        branding::BRANDING_WORDMARK_DARK_URL_KEY => {
            branding::normalize_wordmark_dark_url_config_value(value)
        }
        branding::BRANDING_WORDMARK_LIGHT_URL_KEY => {
            branding::normalize_wordmark_light_url_config_value(value)
        }
        _ => Ok(value.to_string()),
    }
}

fn apply_system_config_definition(mut config: system_config::Model) -> system_config::Model {
    if config.source != "system" {
        return config;
    }

    let Some(def) = ALL_CONFIGS.iter().find(|def| def.key == config.key) else {
        return config;
    };

    config.value_type = def.value_type.to_string();
    config.requires_restart = def.requires_restart;
    config.is_sensitive = def.is_sensitive;
    config.category = def.category.to_string();
    config.description = def.description.to_string();
    config
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicBranding {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub wordmark_dark_url: String,
    pub wordmark_light_url: String,
    pub site_url: Option<String>,
    pub allow_user_registration: bool,
}

pub fn get_public_branding(state: &AppState) -> PublicBranding {
    let auth_policy = auth_runtime::RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    PublicBranding {
        title: branding::title_or_default(&state.runtime_config),
        description: branding::description_or_default(&state.runtime_config),
        favicon_url: branding::favicon_url_or_default(&state.runtime_config),
        wordmark_dark_url: branding::wordmark_dark_url_or_default(&state.runtime_config),
        wordmark_light_url: branding::wordmark_light_url_or_default(&state.runtime_config),
        site_url: site_url::public_site_url(&state.runtime_config),
        allow_user_registration: auth_policy.allow_user_registration,
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
    pub category: String,
    pub description: String,
    pub requires_restart: bool,
    pub is_sensitive: bool,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableItem {
    pub token: String,
    pub label_i18n_key: String,
    pub description_i18n_key: String,
}

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableGroup {
    pub category: String,
    pub template_code: String,
    pub label_i18n_key: String,
    pub variables: Vec<TemplateVariableItem>,
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
            category: def.category.to_string(),
            description: def.description.to_string(),
            requires_restart: def.requires_restart,
            is_sensitive: def.is_sensitive,
        })
        .collect()
}

pub fn list_template_variable_groups() -> Vec<TemplateVariableGroup> {
    crate::services::mail_template::list_template_variable_groups()
        .into_iter()
        .map(|group| TemplateVariableGroup {
            category: group.category,
            template_code: group.template_code,
            label_i18n_key: group.label_i18n_key,
            variables: group
                .variables
                .into_iter()
                .map(|variable| TemplateVariableItem {
                    token: variable.token,
                    label_i18n_key: variable.label_i18n_key,
                    description_i18n_key: variable.description_i18n_key,
                })
                .collect(),
        })
        .collect()
}
