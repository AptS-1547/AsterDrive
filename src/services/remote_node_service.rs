//! 服务模块：`remote_node_service`。

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{managed_follower_repo, policy_repo};
use crate::entities::managed_follower;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::storage::remote_protocol::{
    RemoteStorageCapabilities, RemoteStorageClient, normalize_remote_base_url,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DbErr, Set, SqlErr};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteNodeInfo {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub namespace: String,
    pub is_enabled: bool,
    pub last_error: String,
    pub capabilities: RemoteStorageCapabilities,
    pub last_checked_at: Option<chrono::DateTime<Utc>>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<Utc>,
}

impl From<managed_follower::Model> for RemoteNodeInfo {
    fn from(model: managed_follower::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            base_url: model.base_url,
            namespace: model.namespace,
            is_enabled: model.is_enabled,
            last_error: model.last_error,
            capabilities: parse_capabilities(&model.last_capabilities),
            last_checked_at: model.last_checked_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateRemoteNodeInput {
    pub name: String,
    pub base_url: String,
    pub namespace: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateRemoteNodeInput {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub namespace: Option<String>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TestRemoteNodeInput {
    pub base_url: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteNodeHealthTestStats {
    pub checked: usize,
    pub healthy: usize,
    pub failed: usize,
    pub skipped: usize,
}

pub async fn list_paginated(
    state: &PrimaryAppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<RemoteNodeInfo>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) =
            managed_follower_repo::find_paginated(&state.db, limit, offset).await?;
        Ok((items.into_iter().map(Into::into).collect(), total))
    })
    .await
}

pub async fn get(state: &PrimaryAppState, id: i64) -> Result<RemoteNodeInfo> {
    managed_follower_repo::find_by_id(&state.db, id)
        .await
        .map(Into::into)
}

pub async fn create(state: &PrimaryAppState, input: CreateRemoteNodeInput) -> Result<RemoteNodeInfo> {
    let normalized = normalize_create_input(input)?;
    let (access_key, secret_key) = generate_managed_credentials();
    let now = Utc::now();
    let created = managed_follower::ActiveModel {
        name: Set(normalized.name),
        base_url: Set(normalized.base_url),
        access_key: Set(access_key),
        secret_key: Set(secret_key),
        namespace: Set(normalized.namespace),
        is_enabled: Set(normalized.is_enabled),
        last_capabilities: Set("{}".to_string()),
        last_error: Set(String::new()),
        last_checked_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.db)
    .await
    .map_err(map_remote_node_db_err)?;

    refresh_registry(state).await?;
    Ok(created.into())
}

pub async fn update(
    state: &PrimaryAppState,
    id: i64,
    input: UpdateRemoteNodeInput,
) -> Result<RemoteNodeInfo> {
    let existing = managed_follower_repo::find_by_id(&state.db, id).await?;
    let normalized = normalize_update_input(input)?;

    let mut active: managed_follower::ActiveModel = existing.into();
    if let Some(value) = normalized.name {
        active.name = Set(value);
    }
    if let Some(value) = normalized.base_url {
        active.base_url = Set(value);
    }
    if let Some(value) = normalized.namespace {
        active.namespace = Set(value);
    }
    if let Some(value) = normalized.is_enabled {
        active.is_enabled = Set(value);
    }
    active.updated_at = Set(Utc::now());

    let updated = active
        .update(&state.db)
        .await
        .map_err(map_remote_node_db_err)?;
    refresh_registry(state).await?;
    Ok(updated.into())
}

pub async fn delete(state: &PrimaryAppState, id: i64) -> Result<()> {
    let policy_refs = policy_repo::count_by_remote_node_id(&state.db, id).await?;
    if policy_refs > 0 {
        return Err(AsterError::validation_error(format!(
            "cannot delete remote node: {policy_refs} storage policy(s) still reference it"
        )));
    }
    managed_follower_repo::delete(&state.db, id).await?;
    refresh_registry(state).await?;
    Ok(())
}

pub async fn test_connection(state: &PrimaryAppState, id: i64) -> Result<RemoteNodeInfo> {
    let node = managed_follower_repo::find_by_id(&state.db, id).await?;
    let updated = probe_and_persist_node(state, &node).await?;
    refresh_registry(state).await?;
    Ok(updated.into())
}

pub async fn test_connection_params(
    input: TestRemoteNodeInput,
) -> Result<RemoteStorageCapabilities> {
    probe_connection(&input)
        .await
        .map_err(map_connection_test_error)
}

pub async fn run_health_tests(state: &PrimaryAppState) -> Result<RemoteNodeHealthTestStats> {
    let nodes = managed_follower_repo::find_all(&state.db).await?;
    let mut stats = RemoteNodeHealthTestStats::default();

    for node in nodes {
        if !node.is_enabled || node.base_url.trim().is_empty() {
            stats.skipped += 1;
            continue;
        }

        let updated = probe_and_persist_node(state, &node).await?;
        stats.checked += 1;
        if updated.last_error.is_empty() {
            stats.healthy += 1;
        } else {
            stats.failed += 1;
        }
    }

    if stats.checked > 0 {
        refresh_registry(state).await?;
    }

    Ok(stats)
}

pub fn parse_capabilities(raw: &str) -> RemoteStorageCapabilities {
    serde_json::from_str(raw).unwrap_or_default()
}

pub fn serialize_capabilities(capabilities: &RemoteStorageCapabilities) -> String {
    serde_json::to_string(capabilities).unwrap_or_else(|_| "{}".to_string())
}

async fn probe_connection(input: &TestRemoteNodeInput) -> Result<RemoteStorageCapabilities> {
    let client = RemoteStorageClient::new(&input.base_url, &input.access_key, &input.secret_key)?;
    client.probe_capabilities().await
}

async fn probe_and_persist_node(
    state: &PrimaryAppState,
    node: &managed_follower::Model,
) -> Result<managed_follower::Model> {
    let capabilities = probe_connection(&TestRemoteNodeInput {
        base_url: node.base_url.clone(),
        access_key: node.access_key.clone(),
        secret_key: node.secret_key.clone(),
    })
    .await;

    let (last_capabilities, last_error) = match capabilities {
        Ok(capabilities) => (serialize_capabilities(&capabilities), String::new()),
        Err(error) => ("{}".to_string(), error.message().to_string()),
    };
    managed_follower_repo::touch_probe_result(
        &state.db,
        node.id,
        last_capabilities,
        last_error,
        Some(Utc::now()),
    )
    .await
}

fn normalize_create_input(input: CreateRemoteNodeInput) -> Result<CreateRemoteNodeInput> {
    Ok(CreateRemoteNodeInput {
        name: normalize_non_blank("name", &input.name)?,
        base_url: normalize_remote_base_url(&input.base_url)?,
        namespace: normalize_namespace(&input.namespace)?,
        is_enabled: input.is_enabled,
    })
}

fn generate_managed_credentials() -> (String, String) {
    (
        format!("rn_{}", crate::utils::id::new_short_token()),
        format!(
            "rns_{}{}",
            crate::utils::id::new_short_token(),
            crate::utils::id::new_short_token()
        ),
    )
}

fn normalize_update_input(input: UpdateRemoteNodeInput) -> Result<UpdateRemoteNodeInput> {
    Ok(UpdateRemoteNodeInput {
        name: input
            .name
            .as_deref()
            .map(|value| normalize_non_blank("name", value))
            .transpose()?,
        base_url: input
            .base_url
            .as_deref()
            .map(normalize_remote_base_url)
            .transpose()?,
        namespace: input
            .namespace
            .as_deref()
            .map(normalize_namespace)
            .transpose()?,
        is_enabled: input.is_enabled,
    })
}

fn normalize_non_blank(field: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_namespace(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error("namespace cannot be blank"));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AsterError::validation_error(
            "namespace only allows ASCII letters, digits, '.', '_' and '-'",
        ));
    }
    Ok(trimmed.to_string())
}

async fn refresh_registry(state: &PrimaryAppState) -> Result<()> {
    state
        .driver_registry
        .reload_managed_followers(&state.db)
        .await?;
    state.driver_registry.invalidate_all();
    Ok(())
}

fn map_remote_node_db_err(error: DbErr) -> AsterError {
    if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        AsterError::validation_error("remote node unique field conflict")
    } else {
        AsterError::from(error)
    }
}

fn map_connection_test_error(error: AsterError) -> AsterError {
    match error {
        AsterError::StorageDriverError(message) => AsterError::validation_error(message),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateRemoteNodeInput, UpdateRemoteNodeInput, generate_managed_credentials,
        normalize_create_input, normalize_update_input,
    };

    #[test]
    fn normalize_create_input_ignores_managed_credentials() {
        let normalized = normalize_create_input(CreateRemoteNodeInput {
            name: " Edge ".to_string(),
            base_url: " https://remote.example.com/ ".to_string(),
            namespace: "tenant-a".to_string(),
            is_enabled: true,
        })
        .unwrap();

        assert_eq!(normalized.name, "Edge");
        assert_eq!(normalized.base_url, "https://remote.example.com");
        assert_eq!(normalized.namespace, "tenant-a");
        assert!(normalized.is_enabled);
    }

    #[test]
    fn generate_managed_credentials_returns_prefixed_values() {
        let (access_key, secret_key) = generate_managed_credentials();

        assert!(access_key.starts_with("rn_"));
        assert!(secret_key.starts_with("rns_"));
        assert!(access_key.len() > 3);
        assert!(secret_key.len() > 4);
    }

    #[test]
    fn normalize_update_input_preserves_non_credential_fields() {
        let normalized = normalize_update_input(UpdateRemoteNodeInput {
            name: Some(" Edge ".to_string()),
            base_url: Some(" https://remote.example.com/ ".to_string()),
            namespace: Some("tenant-a".to_string()),
            is_enabled: Some(true),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(normalized.name.as_deref(), Some("Edge"));
        assert_eq!(
            normalized.base_url.as_deref(),
            Some("https://remote.example.com")
        );
        assert_eq!(normalized.namespace.as_deref(), Some("tenant-a"));
        assert_eq!(normalized.is_enabled, Some(true));
    }
}
