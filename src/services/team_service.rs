use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{IntoActiveModel, Set, TransactionTrait};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::db::repository::{team_member_repo, team_repo, user_repo};
use crate::entities::{team, team_member, user};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
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

fn validate_team_name(name: &str) -> Result<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AsterError::validation_error("team name cannot be empty"));
    }
    if normalized.len() > 128 {
        return Err(AsterError::validation_error(
            "team name must be at most 128 characters",
        ));
    }
    Ok(normalized.to_string())
}

fn normalize_description(description: Option<&str>) -> String {
    description.unwrap_or_default().trim().to_string()
}

fn default_team_storage_quota(state: &AppState) -> i64 {
    state
        .runtime_config
        .get_i64("default_storage_quota")
        .unwrap_or_else(|| {
            if let Some(raw) = state.runtime_config.get("default_storage_quota") {
                tracing::warn!("invalid default_storage_quota value '{}', using 0", raw);
            }
            0
        })
}

async fn build_team_info(
    state: &AppState,
    team: &team::Model,
    my_role: TeamMemberRole,
) -> Result<TeamInfo> {
    let creator = user_repo::find_by_id(&state.db, team.created_by).await?;
    let member_count = team_member_repo::count_by_team(&state.db, team.id).await?;

    Ok(build_team_info_with_metadata(
        team,
        my_role,
        creator.username,
        member_count,
    ))
}

fn build_team_info_with_metadata(
    team: &team::Model,
    my_role: TeamMemberRole,
    created_by_username: String,
    member_count: u64,
) -> TeamInfo {
    TeamInfo {
        id: team.id,
        name: team.name.clone(),
        description: team.description.clone(),
        created_by: team.created_by,
        created_by_username,
        my_role,
        member_count,
        storage_used: team.storage_used,
        storage_quota: team.storage_quota,
        policy_group_id: team.policy_group_id,
        created_at: team.created_at,
        updated_at: team.updated_at,
        archived_at: team.archived_at,
    }
}

fn build_team_member_info(membership: team_member::Model, user: user::Model) -> TeamMemberInfo {
    TeamMemberInfo {
        id: membership.id,
        team_id: membership.team_id,
        user_id: user.id,
        username: user.username,
        email: user.email,
        status: user.status,
        role: membership.role,
        created_at: membership.created_at,
        updated_at: membership.updated_at,
    }
}

async fn resolve_target_user(
    state: &AppState,
    user_id: Option<i64>,
    identifier: Option<&str>,
) -> Result<user::Model> {
    match (user_id, identifier.map(str::trim).filter(|s| !s.is_empty())) {
        (Some(_), Some(_)) => Err(AsterError::validation_error(
            "specify either user_id or identifier, not both",
        )),
        (None, None) => Err(AsterError::validation_error(
            "user_id or identifier is required",
        )),
        (Some(user_id), None) => user_repo::find_by_id(&state.db, user_id).await,
        (None, Some(identifier)) => {
            if let Some(user) = user_repo::find_by_username(&state.db, identifier).await? {
                return Ok(user);
            }
            if let Some(user) = user_repo::find_by_email(&state.db, identifier).await? {
                return Ok(user);
            }
            Err(AsterError::record_not_found(format!("user '{identifier}'")))
        }
    }
}

async fn require_team_membership(
    state: &AppState,
    team_id: i64,
    user_id: i64,
) -> Result<(team::Model, team_member::Model)> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    Ok((team, membership))
}

fn ensure_can_manage_team(role: TeamMemberRole) -> Result<()> {
    if !role.can_manage_team() {
        return Err(AsterError::auth_forbidden(
            "team owner or admin role is required",
        ));
    }
    Ok(())
}

async fn ensure_not_last_owner(state: &AppState, team_id: i64) -> Result<()> {
    let owner_count =
        team_member_repo::count_by_team_and_role(&state.db, team_id, TeamMemberRole::Owner).await?;
    if owner_count <= 1 {
        return Err(AsterError::validation_error(
            "team must keep at least one owner",
        ));
    }
    Ok(())
}

pub async fn list_teams(state: &AppState, user_id: i64) -> Result<Vec<TeamInfo>> {
    let memberships = team_member_repo::list_by_user_with_team(&state.db, user_id).await?;
    if memberships.is_empty() {
        return Ok(vec![]);
    }

    let creator_ids: Vec<i64> = memberships
        .iter()
        .map(|(_, team)| team.created_by)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let team_ids: Vec<i64> = memberships
        .iter()
        .map(|(_, team)| team.id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let (creators, member_counts) = tokio::try_join!(
        user_repo::find_by_ids(&state.db, &creator_ids),
        team_member_repo::count_by_team_ids(&state.db, &team_ids),
    )?;
    let creator_usernames: HashMap<i64, String> = creators
        .into_iter()
        .map(|creator| (creator.id, creator.username))
        .collect();

    let mut teams = Vec::with_capacity(memberships.len());
    for (membership, team) in memberships {
        let created_by_username = creator_usernames
            .get(&team.created_by)
            .cloned()
            .ok_or_else(|| AsterError::record_not_found(format!("user #{}", team.created_by)))?;
        let member_count = member_counts.get(&team.id).copied().unwrap_or_default();
        teams.push(build_team_info_with_metadata(
            &team,
            membership.role,
            created_by_username,
            member_count,
        ));
    }
    Ok(teams)
}

pub async fn create_team(
    state: &AppState,
    creator_user_id: i64,
    input: CreateTeamInput,
) -> Result<TeamInfo> {
    let name = validate_team_name(&input.name)?;
    let description = normalize_description(input.description.as_deref());
    let policy_group_id = state
        .policy_snapshot
        .system_default_policy_group()
        .map(|group| group.id)
        .ok_or_else(|| {
            AsterError::storage_policy_not_found(
                "no system default storage policy group configured",
            )
        })?;
    let storage_quota = default_team_storage_quota(state);
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let created_team = team_repo::create(
        &txn,
        team::ActiveModel {
            name: Set(name),
            description: Set(description),
            created_by: Set(creator_user_id),
            storage_used: Set(0),
            storage_quota: Set(storage_quota),
            policy_group_id: Set(Some(policy_group_id)),
            created_at: Set(now),
            updated_at: Set(now),
            archived_at: Set(None),
            ..Default::default()
        },
    )
    .await?;
    let membership = team_member_repo::create(
        &txn,
        team_member::ActiveModel {
            team_id: Set(created_team.id),
            user_id: Set(creator_user_id),
            role: Set(TeamMemberRole::Owner),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    build_team_info(state, &created_team, membership.role).await
}

pub async fn get_team(state: &AppState, team_id: i64, user_id: i64) -> Result<TeamInfo> {
    let (team, membership) = require_team_membership(state, team_id, user_id).await?;
    build_team_info(state, &team, membership.role).await
}

pub async fn update_team(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    input: UpdateTeamInput,
) -> Result<TeamInfo> {
    let (team, membership) = require_team_membership(state, team_id, actor_user_id).await?;
    ensure_can_manage_team(membership.role)?;

    let mut active = team.into_active_model();
    if let Some(name) = input.name {
        active.name = Set(validate_team_name(&name)?);
    }
    if let Some(description) = input.description {
        active.description = Set(normalize_description(Some(&description)));
    }
    active.updated_at = Set(Utc::now());

    let updated = team_repo::update(&state.db, active).await?;
    build_team_info(state, &updated, membership.role).await
}

pub async fn archive_team(state: &AppState, team_id: i64, actor_user_id: i64) -> Result<()> {
    let (team, membership) = require_team_membership(state, team_id, actor_user_id).await?;
    if !membership.role.is_owner() {
        return Err(AsterError::auth_forbidden("team owner role is required"));
    }

    let mut active = team.into_active_model();
    let now = Utc::now();
    active.archived_at = Set(Some(now));
    active.updated_at = Set(now);
    team_repo::update(&state.db, active).await?;
    Ok(())
}

pub async fn list_members(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
) -> Result<Vec<TeamMemberInfo>> {
    require_team_membership(state, team_id, actor_user_id).await?;
    let rows = team_member_repo::list_by_team_with_user(&state.db, team_id).await?;
    Ok(rows
        .into_iter()
        .map(|(membership, user)| build_team_member_info(membership, user))
        .collect())
}

pub async fn add_member(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    input: AddTeamMemberInput,
) -> Result<TeamMemberInfo> {
    let (_, actor_membership) = require_team_membership(state, team_id, actor_user_id).await?;
    ensure_can_manage_team(actor_membership.role)?;
    if !actor_membership.role.is_owner() && input.role.is_owner() {
        return Err(AsterError::auth_forbidden(
            "only a team owner can assign owner role",
        ));
    }

    let target_user =
        resolve_target_user(state, input.user_id, input.identifier.as_deref()).await?;
    if !target_user.status.is_active() {
        return Err(AsterError::validation_error(
            "cannot add a disabled user to a team",
        ));
    }
    if team_member_repo::find_by_team_and_user(&state.db, team_id, target_user.id)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(
            "user is already a team member",
        ));
    }

    let now = Utc::now();
    let membership = team_member_repo::create(
        &state.db,
        team_member::ActiveModel {
            team_id: Set(team_id),
            user_id: Set(target_user.id),
            role: Set(input.role),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    Ok(build_team_member_info(membership, target_user))
}

pub async fn update_member_role(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    member_user_id: i64,
    role: TeamMemberRole,
) -> Result<TeamMemberInfo> {
    let (_, actor_membership) = require_team_membership(state, team_id, actor_user_id).await?;
    ensure_can_manage_team(actor_membership.role)?;

    let target_membership =
        team_member_repo::find_by_team_and_user(&state.db, team_id, member_user_id)
            .await?
            .ok_or_else(|| {
                AsterError::record_not_found(format!("team member user #{member_user_id}"))
            })?;

    if !actor_membership.role.is_owner() && (target_membership.role.is_owner() || role.is_owner()) {
        return Err(AsterError::auth_forbidden(
            "only a team owner can manage owner memberships",
        ));
    }

    if target_membership.role.is_owner() && !role.is_owner() {
        ensure_not_last_owner(state, team_id).await?;
    }

    let mut active = target_membership.clone().into_active_model();
    active.role = Set(role);
    active.updated_at = Set(Utc::now());
    let updated = team_member_repo::update(&state.db, active).await?;
    let target_user = user_repo::find_by_id(&state.db, member_user_id).await?;
    Ok(build_team_member_info(updated, target_user))
}

pub async fn remove_member(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    member_user_id: i64,
) -> Result<()> {
    let (_, actor_membership) = require_team_membership(state, team_id, actor_user_id).await?;
    let target_membership =
        team_member_repo::find_by_team_and_user(&state.db, team_id, member_user_id)
            .await?
            .ok_or_else(|| {
                AsterError::record_not_found(format!("team member user #{member_user_id}"))
            })?;

    if actor_user_id != member_user_id {
        ensure_can_manage_team(actor_membership.role)?;
        if !actor_membership.role.is_owner() && target_membership.role.is_owner() {
            return Err(AsterError::auth_forbidden(
                "only a team owner can remove an owner",
            ));
        }
    }

    if target_membership.role.is_owner() {
        ensure_not_last_owner(state, team_id).await?;
    }

    team_member_repo::delete(&state.db, target_membership.id).await
}
