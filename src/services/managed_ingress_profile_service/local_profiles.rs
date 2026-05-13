use chrono::Utc;
use sea_orm::Set;

use crate::db::repository::managed_ingress_profile_repo;
use crate::entities::{managed_ingress_profile, master_binding};
use crate::errors::{AsterError, Result, precondition_failed_with_subcode};
use crate::runtime::FollowerRuntimeState;
use crate::storage::remote_protocol::{
    RemoteCreateIngressProfileRequest, RemoteIngressProfileInfo, RemoteUpdateIngressProfileRequest,
};

use super::driver::{build_driver_from_profile, validate_driver_from_profile};
use super::models::ResolvedIngressTarget;
use super::normalization::{new_profile_key, normalize_create_input, normalize_update_input};

pub async fn list<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
) -> Result<Vec<RemoteIngressProfileInfo>> {
    Ok(
        managed_ingress_profile_repo::find_all_by_binding(state.db(), binding.id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    )
}

pub async fn create<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
    input: RemoteCreateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    let normalized = normalize_create_input(input)?;
    let profile_id = crate::db::transaction::with_transaction(state.db(), async |txn| {
        let should_set_default = normalized.is_default == Some(true)
            || managed_ingress_profile_repo::count_by_binding(txn, binding.id).await? == 0;
        let now = Utc::now();
        let created = managed_ingress_profile_repo::create(
            txn,
            managed_ingress_profile::ActiveModel {
                master_binding_id: Set(binding.id),
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
            managed_ingress_profile_repo::set_only_default_for_binding(txn, binding.id, created.id)
                .await?;
        }
        Ok(created.id)
    })
    .await?;
    let profile = managed_ingress_profile_repo::find_by_id(state.db(), profile_id).await?;
    Ok(reconcile_profile(state, profile).await?.into())
}

pub async fn update<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
    profile_key: &str,
    input: RemoteUpdateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    let existing = find_profile_or_err(state, binding.id, profile_key).await?;
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
            managed_ingress_profile_repo::set_only_default_for_binding(txn, binding.id, updated.id)
                .await?;
        }
        Ok(updated.id)
    })
    .await?;
    let profile = managed_ingress_profile_repo::find_by_id(state.db(), profile_id).await?;
    Ok(reconcile_profile(state, profile).await?.into())
}

pub async fn delete<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
    profile_key: &str,
) -> Result<()> {
    let existing = find_profile_or_err(state, binding.id, profile_key).await?;
    let count = managed_ingress_profile_repo::count_by_binding(state.db(), binding.id).await?;
    if existing.is_default && count > 1 {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_delete_requires_replacement",
            "cannot delete the default managed ingress profile while other profiles still exist; set another profile as default first",
        ));
    }
    managed_ingress_profile_repo::delete_by_binding_and_profile_key(
        state.db(),
        binding.id,
        &existing.profile_key,
    )
    .await
}

pub async fn resolve_effective_target<S: FollowerRuntimeState>(
    state: &S,
    binding: &master_binding::Model,
) -> Result<ResolvedIngressTarget> {
    let profiles =
        managed_ingress_profile_repo::find_all_by_binding(state.db(), binding.id).await?;
    if profiles.is_empty() {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.required",
            "managed ingress profile is required before follower can accept remote writes",
        ));
    }

    let profile = managed_ingress_profile_repo::find_default_by_binding(state.db(), binding.id)
        .await?
        .ok_or_else(|| {
            precondition_failed_with_subcode(
                "managed_ingress.default_missing",
                "managed ingress profiles exist but no default profile is configured",
            )
        })?;
    if !profile.last_error.trim().is_empty() {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_error",
            format!(
                "managed ingress profile '{}' is not ready: {}",
                profile.profile_key, profile.last_error
            ),
        ));
    }
    if profile.applied_revision < profile.desired_revision {
        return Err(precondition_failed_with_subcode(
            "managed_ingress.default_not_applied",
            format!(
                "managed ingress profile '{}' is pending apply",
                profile.profile_key
            ),
        ));
    }

    let driver = build_driver_from_profile(state, &profile)?;
    Ok(ResolvedIngressTarget {
        driver,
        max_file_size: profile.max_file_size,
    })
}

async fn find_profile_or_err<S: FollowerRuntimeState>(
    state: &S,
    master_binding_id: i64,
    profile_key: &str,
) -> Result<managed_ingress_profile::Model> {
    managed_ingress_profile_repo::find_by_binding_and_profile_key(
        state.db(),
        master_binding_id,
        profile_key,
    )
    .await?
    .ok_or_else(|| AsterError::record_not_found(format!("managed_ingress_profile '{profile_key}'")))
}

async fn reconcile_profile<S: FollowerRuntimeState>(
    state: &S,
    profile: managed_ingress_profile::Model,
) -> Result<managed_ingress_profile::Model> {
    let apply_result = validate_driver_from_profile(state, &profile);

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
