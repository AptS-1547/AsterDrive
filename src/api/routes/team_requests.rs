use crate::types::{TeamMemberRole, UserStatus};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AddTeamMemberReq {
    pub user_id: Option<i64>,
    pub identifier: Option<String>,
    pub role: Option<TeamMemberRole>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchTeamMemberReq {
    pub role: TeamMemberRole,
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct ListTeamMembersQuery {
    pub keyword: Option<String>,
    pub role: Option<TeamMemberRole>,
    pub status: Option<UserStatus>,
}
