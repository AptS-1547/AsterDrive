use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::config::definitions::ALL_CONFIGS;
use crate::config::system_config as shared_system_config;
use crate::db::repository::config_repo;
use crate::entities::system_config;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::audit_service::{self, AuditContext};
use crate::types::{SystemConfigSource, SystemConfigValueType};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SystemConfig {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub value_type: SystemConfigValueType,
    pub requires_restart: bool,
    pub is_sensitive: bool,
    pub source: SystemConfigSource,
    pub namespace: String,
    pub category: String,
    pub description: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: Option<i64>,
}

impl From<system_config::Model> for SystemConfig {
    fn from(model: system_config::Model) -> Self {
        Self {
            id: model.id,
            key: model.key,
            value: model.value,
            value_type: model.value_type,
            requires_restart: model.requires_restart,
            is_sensitive: model.is_sensitive,
            source: model.source,
            namespace: model.namespace,
            category: model.category,
            description: model.description,
            updated_at: model.updated_at,
            updated_by: model.updated_by,
        }
    }
}

pub async fn list_paginated(
    state: &PrimaryAppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<SystemConfig>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        config_repo::find_paginated(&state.db, limit, offset).await
    })
    .await?;
    let items = page
        .items
        .into_iter()
        .map(apply_system_config_definition)
        .map(Into::into)
        .collect();
    Ok(OffsetPage::new(items, page.total, page.limit, page.offset))
}

pub async fn get_by_key(state: &PrimaryAppState, key: &str) -> Result<SystemConfig> {
    config_repo::find_by_key(&state.db, key)
        .await?
        .map(apply_system_config_definition)
        .map(Into::into)
        .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))
}

pub async fn set(
    state: &PrimaryAppState,
    key: &str,
    value: &str,
    updated_by: i64,
) -> Result<SystemConfig> {
    let mut normalized_value = value.to_string();

    if let Some(def) = ALL_CONFIGS.iter().find(|def| def.key == key) {
        validate_value_type(def.value_type, value)?;
        normalized_value = normalize_system_value(state, key, value)?;
    }

    let config = apply_system_config_definition(
        config_repo::upsert(&state.db, key, &normalized_value, updated_by).await?,
    );
    state.runtime_config.apply(config.clone());
    Ok(config.into())
}

pub async fn delete(state: &PrimaryAppState, key: &str) -> Result<()> {
    config_repo::delete_by_key(&state.db, key).await?;
    state.runtime_config.remove(key);
    Ok(())
}

pub async fn set_with_audit(
    state: &PrimaryAppState,
    key: &str,
    value: &str,
    updated_by: i64,
    audit_ctx: &AuditContext,
) -> Result<SystemConfig> {
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

fn validate_value_type(value_type: SystemConfigValueType, value: &str) -> Result<()> {
    shared_system_config::validate_value_type(value_type, value)
}

fn normalize_system_value(state: &PrimaryAppState, key: &str, value: &str) -> Result<String> {
    shared_system_config::normalize_system_value(&state.runtime_config, key, value)
}

fn apply_system_config_definition(config: system_config::Model) -> system_config::Model {
    shared_system_config::apply_definition(config)
}
