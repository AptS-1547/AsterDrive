//! 服务模块：`managed_ingress_profile_service`。

use crate::db::repository::{
    managed_follower_repo, managed_ingress_profile_repo, master_binding_repo,
};
use crate::entities::{managed_ingress_profile, master_binding, storage_policy};
use crate::errors::{
    AsterError, MapAsterErr, Result, precondition_failed_with_subcode,
    validation_error_with_subcode,
};
use crate::runtime::{FollowerRuntimeState, PrimaryRuntimeState};
use crate::storage::driver::StorageDriver;
use crate::storage::drivers::{
    local::LocalDriver, s3::S3Driver, s3_config::normalize_s3_endpoint_and_bucket,
};
use crate::storage::remote_protocol::{
    RemoteCreateIngressProfileRequest, RemoteIngressProfileInfo, RemoteStorageClient,
    RemoteUpdateIngressProfileRequest,
};
use crate::types::{DriverType, StoredStoragePolicyAllowedTypes, StoredStoragePolicyOptions};
use chrono::Utc;
use sea_orm::Set;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResolvedIngressTarget {
    pub driver: Arc<dyn StorageDriver>,
    pub max_file_size: i64,
}

impl From<managed_ingress_profile::Model> for RemoteIngressProfileInfo {
    fn from(model: managed_ingress_profile::Model) -> Self {
        Self {
            profile_key: model.profile_key,
            name: model.name,
            driver_type: model.driver_type,
            endpoint: model.endpoint,
            bucket: model.bucket,
            base_path: model.base_path,
            max_file_size: model.max_file_size,
            is_default: model.is_default,
            desired_revision: model.desired_revision,
            applied_revision: model.applied_revision,
            last_error: model.last_error,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub async fn list<S: FollowerRuntimeState>(
    state: &S,
    access_key: &str,
) -> Result<Vec<RemoteIngressProfileInfo>> {
    ensure_single_primary_binding(state.db(), access_key).await?;
    Ok(managed_ingress_profile_repo::find_all(state.db())
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub async fn create<S: FollowerRuntimeState>(
    state: &S,
    access_key: &str,
    input: RemoteCreateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    ensure_single_primary_binding(state.db(), access_key).await?;
    let normalized = normalize_create_input(input)?;
    let profile_id = crate::db::transaction::with_transaction(state.db(), async |txn| {
        let should_set_default = normalized.is_default == Some(true)
            || managed_ingress_profile_repo::count(txn).await? == 0;
        let now = Utc::now();
        let created = managed_ingress_profile_repo::create(
            txn,
            managed_ingress_profile::ActiveModel {
                profile_key: Set(new_profile_key()),
                name: Set(normalized.name),
                driver_type: Set(normalized.driver_type),
                endpoint: Set(normalized.endpoint),
                bucket: Set(normalized.bucket),
                access_key: Set(normalized.access_key),
                secret_key: Set(normalized.secret_key),
                base_path: Set(normalized.base_path),
                max_file_size: Set(normalized.max_file_size),
                is_default: Set(false),
                desired_revision: Set(1),
                applied_revision: Set(0),
                last_error: Set(String::new()),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
        )
        .await?;
        if should_set_default {
            managed_ingress_profile_repo::set_only_default(txn, created.id).await?;
        }
        Ok(created.id)
    })
    .await?;
    let profile = managed_ingress_profile_repo::find_by_id(state.db(), profile_id).await?;
    Ok(reconcile_profile(state, profile).await?.into())
}

pub async fn update<S: FollowerRuntimeState>(
    state: &S,
    access_key: &str,
    profile_key: &str,
    input: RemoteUpdateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    ensure_single_primary_binding(state.db(), access_key).await?;
    let existing = find_profile_or_err(state, profile_key).await?;
    let normalized = normalize_update_input(existing.clone(), input)?;

    if existing.is_default && normalized.is_default == Some(false) {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_update_requires_replacement",
            "cannot unset the default managed ingress profile directly; set another profile as default first",
        ));
    }

    let profile_id = crate::db::transaction::with_transaction(state.db(), async |txn| {
        let mut active: managed_ingress_profile::ActiveModel = existing.clone().into();
        active.name = Set(normalized.name);
        active.driver_type = Set(normalized.driver_type);
        active.endpoint = Set(normalized.endpoint);
        active.bucket = Set(normalized.bucket);
        active.access_key = Set(normalized.access_key);
        active.secret_key = Set(normalized.secret_key);
        active.base_path = Set(normalized.base_path);
        active.max_file_size = Set(normalized.max_file_size);
        active.desired_revision =
            Set(existing.desired_revision.checked_add(1).ok_or_else(|| {
                AsterError::internal_error("managed ingress desired_revision overflow")
            })?);
        active.updated_at = Set(Utc::now());
        let updated = managed_ingress_profile_repo::update(txn, active).await?;
        if normalized.is_default == Some(true) {
            managed_ingress_profile_repo::set_only_default(txn, updated.id).await?;
        }
        Ok(updated.id)
    })
    .await?;
    let profile = managed_ingress_profile_repo::find_by_id(state.db(), profile_id).await?;
    Ok(reconcile_profile(state, profile).await?.into())
}

pub async fn delete<S: FollowerRuntimeState>(
    state: &S,
    access_key: &str,
    profile_key: &str,
) -> Result<()> {
    ensure_single_primary_binding(state.db(), access_key).await?;
    let existing = find_profile_or_err(state, profile_key).await?;
    let count = managed_ingress_profile_repo::count(state.db()).await?;
    if existing.is_default && count > 1 {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_delete_requires_replacement",
            "cannot delete the default managed ingress profile while other profiles still exist; set another profile as default first",
        ));
    }
    managed_ingress_profile_repo::delete_by_profile_key(state.db(), &existing.profile_key).await
}

pub async fn resolve_effective_target<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
) -> Result<ResolvedIngressTarget> {
    let profiles = managed_ingress_profile_repo::find_all(state.db()).await?;
    if profiles.is_empty() {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.required",
            "managed ingress profile is required before follower can accept remote writes",
        ));
    }

    ensure_single_primary_binding(state.db(), &binding.access_key).await?;
    let profile = managed_ingress_profile_repo::find_default(state.db())
        .await?
        .ok_or_else(|| {
            precondition_failed_with_subcode(
                "managed_ingress.default_missing",
                "managed ingress profiles exist but no default profile is configured",
            )
        })?;
    if profile.applied_revision < profile.desired_revision {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_not_applied",
            format!(
                "managed ingress profile '{}' is pending apply",
                profile.profile_key
            ),
        ));
    }
    if !profile.last_error.trim().is_empty() {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_error",
            format!(
                "managed ingress profile '{}' is not ready: {}",
                profile.profile_key, profile.last_error
            ),
        ));
    }

    let driver = build_driver_from_profile(state, &profile)?;
    Ok(ResolvedIngressTarget {
        driver,
        max_file_size: profile.max_file_size,
    })
}

pub async fn list_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
) -> Result<Vec<RemoteIngressProfileInfo>> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .list_ingress_profiles()
        .await
}

pub async fn create_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    input: RemoteCreateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .create_ingress_profile(&input)
        .await
}

pub async fn update_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    profile_key: &str,
    input: RemoteUpdateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .update_ingress_profile(profile_key, &input)
        .await
}

pub async fn delete_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    profile_key: &str,
) -> Result<()> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .delete_ingress_profile(profile_key)
        .await
}

async fn find_profile_or_err<S: FollowerRuntimeState>(
    state: &S,
    profile_key: &str,
) -> Result<managed_ingress_profile::Model> {
    managed_ingress_profile_repo::find_by_profile_key(state.db(), profile_key)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("managed_ingress_profile '{profile_key}'"))
        })
}

async fn ensure_single_primary_binding<C: sea_orm::ConnectionTrait>(
    db: &C,
    access_key: &str,
) -> Result<()> {
    let bindings = master_binding_repo::find_all(db).await?;
    let enabled_bindings: Vec<_> = bindings
        .into_iter()
        .filter(|binding| binding.is_enabled)
        .collect();
    if enabled_bindings.len() != 1 {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.single_primary_required",
            "managed ingress profiles require exactly one active master binding",
        ));
    }
    if enabled_bindings[0].access_key != access_key {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.binding_mismatch",
            "managed ingress profile requests must come from the sole configured primary",
        ));
    }
    Ok(())
}

fn normalize_create_input(
    input: RemoteCreateIngressProfileRequest,
) -> Result<NormalizedIngressProfileInput> {
    let RemoteCreateIngressProfileRequest {
        name,
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
        max_file_size,
        is_default,
    } = input;

    normalize_profile_fields(
        normalize_non_blank("name", &name)?,
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
        max_file_size,
        Some(is_default),
    )
}

fn normalize_update_input(
    existing: managed_ingress_profile::Model,
    input: RemoteUpdateIngressProfileRequest,
) -> Result<NormalizedIngressProfileInput> {
    let driver_type = input.driver_type.unwrap_or(existing.driver_type);
    let same_driver_type = driver_type == existing.driver_type;
    normalize_profile_fields(
        input
            .name
            .as_deref()
            .map(|value| normalize_non_blank("name", value))
            .transpose()?
            .unwrap_or(existing.name),
        driver_type,
        input.endpoint.unwrap_or_else(|| {
            if same_driver_type {
                existing.endpoint.clone()
            } else {
                String::new()
            }
        }),
        input.bucket.unwrap_or_else(|| {
            if same_driver_type {
                existing.bucket.clone()
            } else {
                String::new()
            }
        }),
        input.access_key.unwrap_or_else(|| {
            if same_driver_type {
                existing.access_key.clone()
            } else {
                String::new()
            }
        }),
        input.secret_key.unwrap_or_else(|| {
            if same_driver_type {
                existing.secret_key.clone()
            } else {
                String::new()
            }
        }),
        input.base_path.unwrap_or_else(|| {
            if same_driver_type {
                existing.base_path.clone()
            } else {
                ".".to_string()
            }
        }),
        input.max_file_size.unwrap_or(existing.max_file_size),
        input.is_default,
    )
}

fn normalize_profile_fields(
    name: String,
    driver_type: DriverType,
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    base_path: String,
    max_file_size: i64,
    is_default: Option<bool>,
) -> Result<NormalizedIngressProfileInput> {
    if max_file_size < 0 {
        return Err(AsterError::validation_error(
            "max_file_size must be non-negative",
        ));
    }

    match driver_type {
        DriverType::Remote => Err(validation_error_with_subcode(
            "managed_ingress.driver_unsupported",
            "managed ingress profiles only support local and s3 drivers",
        )),
        DriverType::Local => Ok(NormalizedIngressProfileInput {
            name,
            driver_type,
            endpoint: String::new(),
            bucket: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: normalize_relative_local_path(&base_path)?,
            max_file_size,
            is_default,
        }),
        DriverType::S3 => {
            let normalized = normalize_s3_endpoint_and_bucket(&endpoint, &bucket)?;
            let access_key = normalize_non_blank("access_key", &access_key)?;
            let secret_key = normalize_non_blank("secret_key", &secret_key)?;
            Ok(NormalizedIngressProfileInput {
                name,
                driver_type,
                endpoint: normalized.endpoint,
                bucket: normalized.bucket,
                access_key,
                secret_key,
                base_path: base_path.trim().trim_matches('/').to_string(),
                max_file_size,
                is_default,
            })
        }
    }
}

async fn reconcile_profile<S: FollowerRuntimeState>(
    state: &S,
    profile: managed_ingress_profile::Model,
) -> Result<managed_ingress_profile::Model> {
    let apply_result = (|| -> Result<()> {
        let _ = build_driver_from_profile(state, &profile)?;
        Ok(())
    })();

    let mut active: managed_ingress_profile::ActiveModel = profile.clone().into();
    match apply_result {
        Ok(()) => {
            active.applied_revision = Set(profile.desired_revision);
            active.last_error = Set(String::new());
        }
        Err(error) => {
            active.last_error = Set(error.message().to_string());
        }
    }
    active.updated_at = Set(Utc::now());
    managed_ingress_profile_repo::update(state.db(), active).await
}

fn build_driver_from_profile<S: FollowerRuntimeState>(
    state: &S,
    profile: &managed_ingress_profile::Model,
) -> Result<Arc<dyn StorageDriver>> {
    let policy = build_policy_model(state, profile)?;
    match policy.driver_type {
        DriverType::Local => {
            let base_path = Path::new(&policy.base_path);
            std::fs::create_dir_all(base_path).map_aster_err_ctx(
                &format!(
                    "create managed ingress local path '{}'",
                    base_path.display()
                ),
                AsterError::storage_driver_error,
            )?;
            Ok(Arc::new(LocalDriver::new(&policy)?))
        }
        DriverType::S3 => Ok(Arc::new(S3Driver::new(&policy)?)),
        DriverType::Remote => Err(AsterError::validation_error(
            "managed ingress profiles do not support the remote driver",
        )),
    }
}

fn build_policy_model<S: FollowerRuntimeState>(
    state: &S,
    profile: &managed_ingress_profile::Model,
) -> Result<storage_policy::Model> {
    let base_path = match profile.driver_type {
        DriverType::Local => resolve_managed_local_path(
            &state.config().server.follower.managed_ingress_local_root,
            &profile.base_path,
        )?
        .to_string_lossy()
        .into_owned(),
        DriverType::S3 => profile.base_path.clone(),
        DriverType::Remote => String::new(),
    };

    Ok(storage_policy::Model {
        id: profile.id,
        name: profile.name.clone(),
        driver_type: profile.driver_type,
        endpoint: profile.endpoint.clone(),
        bucket: profile.bucket.clone(),
        access_key: profile.access_key.clone(),
        secret_key: profile.secret_key.clone(),
        base_path,
        remote_node_id: None,
        max_file_size: profile.max_file_size,
        allowed_types: StoredStoragePolicyAllowedTypes::empty(),
        options: StoredStoragePolicyOptions::empty(),
        is_default: profile.is_default,
        chunk_size: 0,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    })
}

fn resolve_managed_local_path(root: &str, relative: &str) -> Result<PathBuf> {
    let trimmed_root = root.trim();
    if trimmed_root.is_empty() {
        return Err(AsterError::config_error(
            "server.follower.managed_ingress_local_root cannot be empty",
        ));
    }
    let normalized = normalize_relative_local_path(relative)?;
    let root_path = Path::new(trimmed_root);
    Ok(if normalized == "." {
        root_path.to_path_buf()
    } else {
        root_path.join(normalized)
    })
}

fn normalize_relative_local_path(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(
            "base_path cannot be blank for local ingress profiles",
        ));
    }

    let candidate = Path::new(trimmed);
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(validation_error_with_subcode(
                    "managed_ingress.local_path_invalid",
                    "local ingress base_path must stay within server.follower.managed_ingress_local_root",
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(normalized.to_string_lossy().replace('\\', "/"))
    }
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

fn new_profile_key() -> String {
    format!("igp_{}", crate::utils::id::new_short_token())
}

async fn remote_client_for_node<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
) -> Result<RemoteStorageClient> {
    let node = managed_follower_repo::find_by_id(state.db(), remote_node_id).await?;
    RemoteStorageClient::new(&node.base_url, &node.access_key, &node.secret_key)
}

struct NormalizedIngressProfileInput {
    name: String,
    driver_type: DriverType,
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    base_path: String,
    max_file_size: i64,
    is_default: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::normalize_relative_local_path;

    #[test]
    fn normalize_relative_local_path_keeps_normal_segments() {
        let normalized = normalize_relative_local_path(" archive/2026 ").unwrap();
        assert_eq!(normalized, "archive/2026");
    }

    #[test]
    fn normalize_relative_local_path_rejects_escape_attempts() {
        let error = normalize_relative_local_path("../secret").unwrap_err();
        assert!(
            error
                .message()
                .contains("server.follower.managed_ingress_local_root")
        );
    }
}
