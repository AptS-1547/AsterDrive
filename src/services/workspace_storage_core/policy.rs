use crate::db::repository::{team_repo, user_repo};
use crate::entities::team;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::workspace_scope_service::{
    WorkspaceStorageScope, require_team_access, verify_folder_access,
};
use crate::types::{DriverType, parse_storage_policy_options};

pub(crate) async fn load_storage_limits(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
) -> Result<(i64, i64)> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            let user = user_repo::find_by_id(&state.db, user_id).await?;
            Ok((user.storage_used, user.storage_quota))
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            let team = team_repo::find_active_by_id(&state.db, team_id).await?;
            Ok((team.storage_used, team.storage_quota))
        }
    }
}

pub(crate) fn local_content_dedup_enabled(policy: &crate::entities::storage_policy::Model) -> bool {
    policy.driver_type == DriverType::Local
        && parse_storage_policy_options(policy.options.as_ref())
            .content_dedup
            .unwrap_or(false)
}

fn resolve_team_policy_group_id(team: &team::Model) -> Result<i64> {
    team.policy_group_id.ok_or_else(|| {
        AsterError::storage_policy_not_found(format!(
            "no storage policy group assigned to team #{}",
            team.id
        ))
    })
}

pub(crate) async fn resolve_policy_for_size(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    file_size: i64,
) -> Result<crate::entities::storage_policy::Model> {
    // 文件夹级策略覆盖优先级最高。
    // 只有目标文件夹没有显式绑定策略时，才回退到个人默认策略 / 团队策略组。
    if let Some(folder_id) = folder_id {
        let folder = verify_folder_access(state, scope, folder_id).await?;

        if let Some(policy_id) = folder.policy_id {
            return state.policy_snapshot.get_policy_or_err(policy_id);
        }
    }

    match scope {
        WorkspaceStorageScope::Personal { user_id } => state
            .policy_snapshot
            .resolve_user_policy_for_size(user_id, file_size),
        WorkspaceStorageScope::Team {
            team_id,
            actor_user_id,
        } => {
            let team = require_team_access(state, team_id, actor_user_id).await?;
            state
                .policy_snapshot
                .resolve_policy_in_group(resolve_team_policy_group_id(&team)?, file_size)
        }
    }
}
