//! Admin-only DTOs consolidated from `src/api/routes/admin/`.

use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

// ── Users ──────────────────────────────────────────────────────────────────

/// Query parameters for the admin user list.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct AdminUserListQuery {
    pub keyword: Option<String>,
    pub role: Option<crate::types::UserRole>,
    pub status: Option<crate::types::UserStatus>,
}

/// Create a new user (admin operation).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateUserReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// Patch an existing user (admin operation).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchUserReq {
    pub email_verified: Option<bool>,
    pub role: Option<crate::types::UserRole>,
    pub status: Option<crate::types::UserStatus>,
    pub storage_quota: Option<i64>,
    /// Omitted = leave unchanged. Explicit `null` is rejected because this
    /// endpoint only supports assigning a policy group, not unassigning one.
    #[serde(
        default,
        deserialize_with = "crate::api::routes::admin::common::deserialize_non_null_policy_group_id"
    )]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<i64>, nullable = false)
    )]
    pub policy_group_id: Option<i64>,
}

/// Reset a user's password (admin operation).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ResetUserPasswordReq {
    pub password: String,
}

// ── Policies ────────────────────────────────────────────────────────────────

/// Create a storage policy.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreatePolicyReq {
    pub name: String,
    pub driver_type: crate::types::DriverType,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub chunk_size: Option<i64>,
    pub is_default: Option<bool>,
    pub allowed_types: Option<Vec<String>>,
    pub options: Option<crate::types::StoragePolicyOptions>,
}

/// Patch a storage policy.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchPolicyReq {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub chunk_size: Option<i64>,
    pub is_default: Option<bool>,
    pub allowed_types: Option<Vec<String>>,
    pub options: Option<crate::types::StoragePolicyOptions>,
}

/// Test a storage policy connection by parameters (without saving).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TestPolicyParamsReq {
    pub driver_type: crate::types::DriverType,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
}

/// A single item within a policy group.
#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PolicyGroupItemReq {
    pub policy_id: i64,
    pub priority: i32,
    #[serde(default)]
    pub min_file_size: i64,
    #[serde(default)]
    pub max_file_size: i64,
}

/// Create a storage policy group.
#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreatePolicyGroupReq {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    pub items: Vec<PolicyGroupItemReq>,
}

/// Patch a storage policy group.
#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchPolicyGroupReq {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub items: Option<Vec<PolicyGroupItemReq>>,
}

/// Migrate all users from one policy group to another.
#[derive(Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct MigratePolicyGroupUsersReq {
    pub target_group_id: i64,
}

fn default_true() -> bool {
    true
}

// ── Config ─────────────────────────────────────────────────────────────────

/// Set a system configuration value.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetConfigReq {
    pub value: String,
}

/// Execute a config action (e.g., send test email).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ExecuteConfigActionReq {
    pub action: crate::services::config_service::ConfigActionType,
    pub discovery_url: Option<String>,
    pub target_email: Option<String>,
    pub value: Option<String>,
}

/// Response from a config action execution.
#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ExecuteConfigActionResp {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

// ── Admin Teams ─────────────────────────────────────────────────────────────

/// Query parameters for the admin team list.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct AdminTeamListQuery {
    pub keyword: Option<String>,
    pub archived: Option<bool>,
}

/// Create a team (admin operation).
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminCreateTeamReq {
    pub name: String,
    pub description: Option<String>,
    pub admin_user_id: Option<i64>,
    pub admin_identifier: Option<String>,
    pub policy_group_id: Option<i64>,
}

/// Patch a team (admin operation).
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminPatchTeamReq {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::api::routes::admin::common::deserialize_non_null_policy_group_id"
    )]
    pub policy_group_id: Option<i64>,
}

/// Alias for `AdminTeamListQuery` (admin listing query).
pub type AdminListQuery = AdminTeamListQuery;
