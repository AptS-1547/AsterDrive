use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::types::{TeamMemberRole, UserStatus};

#[derive(Debug, Clone)]
pub struct CreateTeamInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTeamInput {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddTeamMemberInput {
    pub user_id: Option<i64>,
    pub identifier: Option<String>,
    pub role: TeamMemberRole,
}

#[derive(Debug, Clone)]
pub struct AdminCreateTeamInput {
    pub name: String,
    pub description: Option<String>,
    pub admin_user_id: Option<i64>,
    pub admin_identifier: Option<String>,
    pub policy_group_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct AdminUpdateTeamInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub policy_group_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TeamInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_by: i64,
    pub created_by_username: String,
    pub my_role: TeamMemberRole,
    pub member_count: u64,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TeamMemberInfo {
    pub id: i64,
    pub team_id: i64,
    pub user_id: i64,
    pub username: String,
    pub email: String,
    pub status: UserStatus,
    pub role: TeamMemberRole,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct TeamMemberListFilters {
    pub keyword: Option<String>,
    pub role: Option<TeamMemberRole>,
    pub status: Option<UserStatus>,
}

impl TeamMemberListFilters {
    pub fn from_inputs(
        keyword: Option<&str>,
        role: Option<TeamMemberRole>,
        status: Option<UserStatus>,
    ) -> Self {
        Self {
            keyword: keyword
                .map(str::trim)
                .filter(|keyword| !keyword.is_empty())
                .map(str::to_lowercase),
            role,
            status,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TeamMemberPage {
    pub items: Vec<TeamMemberInfo>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub owner_count: u64,
    pub manager_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminTeamInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_by: i64,
    pub created_by_username: String,
    pub member_count: u64,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
}
