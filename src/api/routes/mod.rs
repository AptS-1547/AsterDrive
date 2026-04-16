use crate::services::workspace_storage_service::WorkspaceStorageScope;

pub mod admin;
pub mod auth;
pub mod batch;
pub mod files;
pub mod folders;
pub mod frontend;
pub mod health;
pub mod properties;
pub mod public;
pub mod search;
pub mod share_public;
pub mod shares;
pub mod tasks;
pub mod team_batch;
pub mod team_requests;
pub mod team_search;
pub mod team_shares;
pub mod team_space;
pub mod team_tasks;
pub mod team_trash;
pub mod teams;
pub mod trash;
pub mod webdav_accounts;
pub mod wopi;

pub(crate) fn team_scope(team_id: i64, user_id: i64) -> WorkspaceStorageScope {
    WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    }
}
