//! Team membership request DTOs (shared between user and admin team routes).
//!
//! NOTE: These types are defined in `crate::api::dto::teams` and re-exported here
//! for backwards compatibility with existing imports.

pub use crate::api::dto::teams::{AddTeamMemberReq, ListTeamMembersQuery, PatchTeamMemberReq};
