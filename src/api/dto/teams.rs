//! `teams` API DTO 定义。

use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

// ── Team CRUD ───────────────────────────────────────────────────────────────

/// Query parameters for listing teams.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct ListTeamsQuery {
    pub archived: Option<bool>,
}

/// Create a new team.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateTeamReq {
    pub name: String,
    pub description: Option<String>,
}

/// Patch (partial update) a team.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchTeamReq {
    pub name: Option<String>,
    pub description: Option<String>,
}

// ── Team membership ──────────────────────────────────────────────────────────

/// Add a user to a team.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AddTeamMemberReq {
    pub user_id: Option<i64>,
    pub identifier: Option<String>,
    pub role: Option<crate::types::TeamMemberRole>,
}

/// Patch a team member's role.
#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchTeamMemberReq {
    pub role: crate::types::TeamMemberRole,
}

/// Query parameters for listing team members.
#[derive(Debug, Deserialize, Default)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct ListTeamMembersQuery {
    pub keyword: Option<String>,
    pub role: Option<crate::types::TeamMemberRole>,
    pub status: Option<crate::types::UserStatus>,
}
