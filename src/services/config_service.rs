use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::config::definitions::ALL_CONFIGS;
use crate::db::repository::config_repo;
use crate::entities::system_config;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::audit_service::{self, AuditContext};
use serde::Serialize;
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
    // 系统配置做值类型校验
    if let Some(def) = ALL_CONFIGS.iter().find(|d| d.key == key) {
        validate_value_type(def.value_type, value)?;
    }

    config_repo::upsert(&state.db, key, value, updated_by).await
}

pub async fn delete(state: &AppState, key: &str) -> Result<()> {
    config_repo::delete_by_key(&state.db, key).await
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
    match value_type {
        "boolean" => {
            if value != "true" && value != "false" {
                return Err(AsterError::validation_error(
                    "boolean config must be 'true' or 'false'",
                ));
            }
        }
        "number" => {
            if value.parse::<f64>().is_err() {
                return Err(AsterError::validation_error(
                    "number config must be a valid number",
                ));
            }
        }
        _ => {} // string 不做校验
    }
    Ok(())
}

// ── Config Schema ─────────────────────────────────────────────────────

/// 系统配置的 schema 信息（从 ALL_CONFIGS 生成）
#[derive(Serialize, ToSchema)]
pub struct ConfigSchemaItem {
    pub key: String,
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
            value_type: def.value_type.to_string(),
            default_value: (def.default_fn)(),
            category: def.category.to_string(),
            description: def.description.to_string(),
            requires_restart: def.requires_restart,
            is_sensitive: def.is_sensitive,
        })
        .collect()
}
