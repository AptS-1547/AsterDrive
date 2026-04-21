//! 存储策略服务聚合入口。

mod groups;
mod models;
mod policies;
mod shared;

use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::audit_service::{self, AuditContext};

pub use groups::{
    create_group, delete_group, ensure_policy_groups_seeded, get_group, list_groups_paginated,
    migrate_group_users, update_group,
};
pub use models::{
    CreateStoragePolicyGroupInput, CreateStoragePolicyInput, PolicyGroupUserMigrationResult,
    StoragePolicy, StoragePolicyConnectionInput, StoragePolicyGroupInfo,
    StoragePolicyGroupItemInfo, StoragePolicyGroupItemInput, StoragePolicySummaryInfo,
    UpdateStoragePolicyGroupInput, UpdateStoragePolicyInput,
};
pub use policies::{
    create, delete, get, list_paginated, test_connection, test_connection_params,
    test_default_connection, update,
};

pub async fn create_group_with_audit(
    state: &PrimaryAppState,
    input: CreateStoragePolicyGroupInput,
    audit_ctx: &AuditContext,
) -> Result<StoragePolicyGroupInfo> {
    let group = create_group(state, input).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::AdminCreatePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(group)
}

pub async fn update_group_with_audit(
    state: &PrimaryAppState,
    id: i64,
    input: UpdateStoragePolicyGroupInput,
    audit_ctx: &AuditContext,
) -> Result<StoragePolicyGroupInfo> {
    let group = update_group(state, id, input).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::AdminUpdatePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(group)
}

pub async fn delete_group_with_audit(
    state: &PrimaryAppState,
    id: i64,
    audit_ctx: &AuditContext,
) -> Result<()> {
    let group = get_group(state, id).await?;
    delete_group(state, id).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::AdminDeletePolicyGroup,
        Some("policy_group"),
        Some(group.id),
        Some(&group.name),
        audit_service::details(audit_service::PolicyGroupAuditDetails {
            is_default: group.is_default,
            is_enabled: group.is_enabled,
            item_count: group.items.len(),
        }),
    )
    .await;
    Ok(())
}

pub async fn migrate_group_users_with_audit(
    state: &PrimaryAppState,
    source_group_id: i64,
    target_group_id: i64,
    audit_ctx: &AuditContext,
) -> Result<PolicyGroupUserMigrationResult> {
    let source_group = get_group(state, source_group_id).await?;
    let target_group = get_group(state, target_group_id).await?;
    let result = migrate_group_users(state, source_group_id, target_group_id).await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::AdminMigratePolicyGroupUsers,
        Some("policy_group"),
        Some(source_group.id),
        Some(&source_group.name),
        audit_service::details(audit_service::PolicyGroupMigrationDetails {
            source_group_id: source_group.id,
            source_group_name: &source_group.name,
            target_group_id: target_group.id,
            target_group_name: &target_group.name,
            affected_users: result.affected_users,
            migrated_assignments: result.migrated_assignments,
        }),
    )
    .await;
    Ok(result)
}
