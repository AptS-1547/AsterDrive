use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{ConnectionTrait, IntoActiveModel, Set, TransactionTrait};
use serde::Serialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{
    file_repo, lock_repo, policy_group_repo, share_repo, team_member_repo, team_repo,
    upload_session_repo, user_repo,
};
use crate::entities::{team, team_member, upload_session, user};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::audit_service;
use crate::types::{EntityType, TeamMemberRole, UserStatus};

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

const MISSING_CREATOR_USERNAME: &str = "<deleted_user>";
const DEFAULT_TEAM_ARCHIVE_RETENTION_DAYS: i64 = 7;

fn validate_team_name(name: &str) -> Result<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AsterError::validation_error("team name cannot be empty"));
    }
    if normalized.chars().count() > 128 {
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
    let raw = state.runtime_config.get("default_storage_quota");
    let Some(raw) = raw.as_deref() else {
        return 0;
    };

    match raw.trim().parse::<i64>() {
        Ok(value) if value >= 0 => value,
        Ok(_) => {
            tracing::warn!("invalid default_storage_quota value '{}', using 0", raw);
            0
        }
        Err(_) => {
            tracing::warn!("invalid default_storage_quota value '{}', using 0", raw);
            0
        }
    }
}

fn missing_creator_username(team: &team::Model) -> String {
    tracing::warn!(
        team_id = team.id,
        created_by = team.created_by,
        "team creator missing; using placeholder username"
    );
    MISSING_CREATOR_USERNAME.to_string()
}

async fn load_creator_username(state: &AppState, team: &team::Model) -> Result<String> {
    match user_repo::find_by_id(&state.db, team.created_by).await {
        Ok(creator) => Ok(creator.username),
        Err(AsterError::RecordNotFound(_)) => Ok(missing_creator_username(team)),
        Err(err) => Err(err),
    }
}

async fn build_team_info(
    state: &AppState,
    team: &team::Model,
    my_role: TeamMemberRole,
) -> Result<TeamInfo> {
    let creator_username = load_creator_username(state, team).await?;
    let member_count = team_member_repo::count_by_team(&state.db, team.id).await?;

    Ok(build_team_info_with_metadata(
        team,
        my_role,
        creator_username,
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

async fn build_admin_team_info(state: &AppState, team: &team::Model) -> Result<AdminTeamInfo> {
    let creator_username = load_creator_username(state, team).await?;
    let member_count = team_member_repo::count_by_team(&state.db, team.id).await?;

    Ok(build_admin_team_info_with_metadata(
        team,
        creator_username,
        member_count,
    ))
}

fn build_admin_team_info_with_metadata(
    team: &team::Model,
    created_by_username: String,
    member_count: u64,
) -> AdminTeamInfo {
    AdminTeamInfo {
        id: team.id,
        name: team.name.clone(),
        description: team.description.clone(),
        created_by: team.created_by,
        created_by_username,
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

async fn ensure_not_last_owner<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<()> {
    let owner_count =
        team_member_repo::count_by_team_and_role(db, team_id, TeamMemberRole::Owner).await?;
    if owner_count <= 1 {
        return Err(AsterError::validation_error(
            "team must keep at least one owner",
        ));
    }
    Ok(())
}

async fn ensure_not_last_manager<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<()> {
    let owner_count =
        team_member_repo::count_by_team_and_role(db, team_id, TeamMemberRole::Owner).await?;
    let admin_count =
        team_member_repo::count_by_team_and_role(db, team_id, TeamMemberRole::Admin).await?;
    if owner_count + admin_count <= 1 {
        return Err(AsterError::validation_error(
            "team must keep at least one admin or owner",
        ));
    }
    Ok(())
}

async fn load_team_metadata(
    state: &AppState,
    teams: &[team::Model],
) -> Result<(HashMap<i64, String>, HashMap<i64, u64>)> {
    if teams.is_empty() {
        return Ok((HashMap::new(), HashMap::new()));
    }

    let creator_ids: Vec<i64> = teams
        .iter()
        .map(|team| team.created_by)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let team_ids: Vec<i64> = teams
        .iter()
        .map(|team| team.id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let (creators, member_counts) = tokio::try_join!(
        user_repo::find_by_ids(&state.db, &creator_ids),
        team_member_repo::count_by_team_ids(&state.db, &team_ids),
    )?;

    Ok((
        creators
            .into_iter()
            .map(|creator| (creator.id, creator.username))
            .collect(),
        member_counts,
    ))
}

async fn ensure_assignable_policy_group(state: &AppState, group_id: i64) -> Result<()> {
    let group = policy_group_repo::find_group_by_id(&state.db, group_id).await?;
    if !group.is_enabled {
        return Err(AsterError::validation_error(
            "cannot assign a disabled storage policy group",
        ));
    }

    let items = policy_group_repo::find_group_items(&state.db, group_id).await?;
    if items.is_empty() {
        return Err(AsterError::validation_error(
            "cannot assign a storage policy group without policies",
        ));
    }

    Ok(())
}

async fn resolve_required_policy_group_id(
    state: &AppState,
    policy_group_id: Option<i64>,
) -> Result<i64> {
    let group_id = match policy_group_id {
        Some(group_id) => group_id,
        None => state
            .policy_snapshot
            .system_default_policy_group()
            .map(|group| group.id)
            .ok_or_else(|| {
                AsterError::validation_error(
                    "no system default storage policy group configured; provide policy_group_id when creating a team",
                )
            })?,
    };

    ensure_assignable_policy_group(state, group_id).await?;
    Ok(group_id)
}

async fn create_team_record(
    state: &AppState,
    created_by_user_id: i64,
    initial_member_user_id: i64,
    initial_member_role: TeamMemberRole,
    input: CreateTeamInput,
    policy_group_id: i64,
) -> Result<team::Model> {
    let name = validate_team_name(&input.name)?;
    let description = normalize_description(input.description.as_deref());
    let storage_quota = default_team_storage_quota(state);
    let now = Utc::now();

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let created_team = team_repo::create(
        &txn,
        team::ActiveModel {
            name: Set(name),
            description: Set(description),
            created_by: Set(created_by_user_id),
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
    team_member_repo::create(
        &txn,
        team_member::ActiveModel {
            team_id: Set(created_team.id),
            user_id: Set(initial_member_user_id),
            role: Set(initial_member_role),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(created_team)
}

async fn update_team_record(
    state: &AppState,
    team: team::Model,
    input: UpdateTeamInput,
    policy_group_id: Option<i64>,
) -> Result<team::Model> {
    let mut active = team.into_active_model();
    if let Some(name) = input.name {
        active.name = Set(validate_team_name(&name)?);
    }
    if let Some(description) = input.description {
        active.description = Set(normalize_description(Some(&description)));
    }
    if let Some(policy_group_id) = policy_group_id {
        ensure_assignable_policy_group(state, policy_group_id).await?;
        active.policy_group_id = Set(Some(policy_group_id));
    }
    active.updated_at = Set(Utc::now());

    team_repo::update(&state.db, active).await
}

async fn archive_team_record(state: &AppState, team: team::Model) -> Result<()> {
    let mut active = team.into_active_model();
    let now = Utc::now();
    active.archived_at = Set(Some(now));
    active.updated_at = Set(now);
    team_repo::update(&state.db, active).await?;
    Ok(())
}

async fn restore_team_record(state: &AppState, team: team::Model) -> Result<team::Model> {
    let mut active = team.into_active_model();
    let now = Utc::now();
    active.archived_at = Set(None);
    active.updated_at = Set(now);
    team_repo::update(&state.db, active).await
}

fn load_team_archive_retention_days(state: &AppState) -> i64 {
    let Some(raw) = state.runtime_config.get("team_archive_retention_days") else {
        return DEFAULT_TEAM_ARCHIVE_RETENTION_DAYS;
    };

    match raw.trim().parse::<i64>() {
        Ok(value) if value >= 0 => value,
        Ok(_) | Err(_) => {
            tracing::warn!(
                "invalid team_archive_retention_days value '{}', using default",
                raw
            );
            DEFAULT_TEAM_ARCHIVE_RETENTION_DAYS
        }
    }
}

async fn cleanup_team_upload_sessions(
    state: &AppState,
    sessions: Vec<upload_session::Model>,
) -> Result<()> {
    for session in sessions {
        if let Some(temp_key) = session.s3_temp_key.as_deref()
            && let Some(policy) = state.policy_snapshot.get_policy(session.policy_id)
            && let Ok(driver) = state.driver_registry.get_driver(&policy)
        {
            if let Some(multipart_id) = session.s3_multipart_id.as_deref() {
                if let Err(err) = driver.abort_multipart_upload(temp_key, multipart_id).await {
                    tracing::warn!(
                        upload_id = %session.id,
                        "failed to abort team multipart upload during cleanup: {err}"
                    );
                }
            } else if let Err(err) = driver.delete(temp_key).await {
                tracing::warn!(
                    upload_id = %session.id,
                    "failed to delete team temp upload object during cleanup: {err}"
                );
            }
        }

        let temp_dir =
            crate::utils::paths::upload_temp_dir(&state.config.server.upload_temp_dir, &session.id);
        crate::utils::cleanup_temp_dir(&temp_dir).await;
        upload_session_repo::delete(&state.db, &session.id).await?;
    }

    Ok(())
}

async fn clear_team_locks(state: &AppState, team_id: i64) -> Result<()> {
    let prefix = format!("/teams/{team_id}/");
    let locks = lock_repo::find_by_path_prefix(&state.db, &prefix).await?;
    for lock in &locks {
        if let Err(err) = crate::services::lock_service::set_entity_locked(
            &state.db,
            lock.entity_type,
            lock.entity_id,
            false,
        )
        .await
        {
            tracing::warn!(
                lock_id = lock.id,
                team_id,
                "failed to clear team lock flag during cleanup: {err}"
            );
        }
    }
    lock_repo::delete_by_path_prefix(&state.db, &prefix).await?;
    Ok(())
}

async fn force_delete_archived_team(state: &AppState, team: team::Model) -> Result<()> {
    let team_id = team.id;
    let upload_sessions = upload_session_repo::find_by_team(&state.db, team_id).await?;
    cleanup_team_upload_sessions(state, upload_sessions).await?;
    clear_team_locks(state, team_id).await?;
    share_repo::delete_all_by_team(&state.db, team_id).await?;

    let all_files = file_repo::find_all_by_team(&state.db, team_id).await?;
    crate::services::file_service::batch_purge_in_scope(
        state,
        crate::services::workspace_storage_service::WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: team.created_by,
        },
        all_files,
    )
    .await?;

    let all_folders =
        crate::db::repository::folder_repo::find_all_by_team(&state.db, team_id).await?;
    let folder_ids: Vec<i64> = all_folders.iter().map(|folder| folder.id).collect();
    crate::db::repository::property_repo::delete_all_for_entities(
        &state.db,
        EntityType::Folder,
        &folder_ids,
    )
    .await?;
    crate::db::repository::folder_repo::delete_many(&state.db, &folder_ids).await?;
    team_repo::delete(&state.db, team_id).await?;

    Ok(())
}

pub async fn list_teams(state: &AppState, user_id: i64, archived: bool) -> Result<Vec<TeamInfo>> {
    let memberships = if archived {
        team_member_repo::list_by_user_with_archived_team(&state.db, user_id).await?
    } else {
        team_member_repo::list_by_user_with_team(&state.db, user_id).await?
    };
    if memberships.is_empty() {
        return Ok(vec![]);
    }

    let teams_only: Vec<team::Model> = memberships.iter().map(|(_, team)| team.clone()).collect();
    let (creator_usernames, member_counts) = load_team_metadata(state, &teams_only).await?;

    let mut teams = Vec::with_capacity(memberships.len());
    for (membership, team) in memberships {
        let created_by_username = creator_usernames
            .get(&team.created_by)
            .cloned()
            .unwrap_or_else(|| missing_creator_username(&team));
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
    let policy_group_id = resolve_required_policy_group_id(state, None).await?;
    let created_team = create_team_record(
        state,
        creator_user_id,
        creator_user_id,
        TeamMemberRole::Owner,
        input,
        policy_group_id,
    )
    .await?;
    build_team_info(state, &created_team, TeamMemberRole::Owner).await
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
    let updated = update_team_record(state, team, input, None).await?;
    build_team_info(state, &updated, membership.role).await
}

pub async fn archive_team(state: &AppState, team_id: i64, actor_user_id: i64) -> Result<()> {
    let (team, membership) = require_team_membership(state, team_id, actor_user_id).await?;
    if !membership.role.is_owner() {
        return Err(AsterError::auth_forbidden("team owner role is required"));
    }

    archive_team_record(state, team).await
}

pub async fn restore_team(state: &AppState, team_id: i64, actor_user_id: i64) -> Result<TeamInfo> {
    let team = team_repo::find_archived_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, actor_user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    ensure_can_manage_team(membership.role)?;

    let restored = restore_team_record(state, team).await?;
    build_team_info(state, &restored, membership.role).await
}

pub async fn list_admin_teams(
    state: &AppState,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
    archived: bool,
) -> Result<OffsetPage<AdminTeamInfo>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        if archived {
            team_repo::find_archived_paginated(&state.db, limit, offset, keyword).await
        } else {
            team_repo::find_active_paginated(&state.db, limit, offset, keyword).await
        }
    })
    .await?;
    let (creator_usernames, member_counts) = load_team_metadata(state, &page.items).await?;

    Ok(OffsetPage::new(
        page.items
            .into_iter()
            .map(|team| {
                let created_by_username = creator_usernames
                    .get(&team.created_by)
                    .cloned()
                    .unwrap_or_else(|| missing_creator_username(&team));
                let member_count = member_counts.get(&team.id).copied().unwrap_or_default();
                build_admin_team_info_with_metadata(&team, created_by_username, member_count)
            })
            .collect(),
        page.total,
        page.limit,
        page.offset,
    ))
}

pub async fn get_admin_team(state: &AppState, team_id: i64) -> Result<AdminTeamInfo> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    build_admin_team_info(state, &team).await
}

pub async fn create_admin_team(
    state: &AppState,
    actor_user_id: i64,
    input: AdminCreateTeamInput,
) -> Result<AdminTeamInfo> {
    let team_admin = resolve_target_user(
        state,
        input.admin_user_id,
        input.admin_identifier.as_deref(),
    )
    .await?;
    if !team_admin.status.is_active() {
        return Err(AsterError::validation_error(
            "cannot create a team for a disabled user",
        ));
    }

    let policy_group_id = resolve_required_policy_group_id(state, input.policy_group_id).await?;
    let created_team = create_team_record(
        state,
        actor_user_id,
        team_admin.id,
        TeamMemberRole::Admin,
        CreateTeamInput {
            name: input.name,
            description: input.description,
        },
        policy_group_id,
    )
    .await?;
    build_admin_team_info(state, &created_team).await
}

pub async fn update_admin_team(
    state: &AppState,
    team_id: i64,
    input: AdminUpdateTeamInput,
) -> Result<AdminTeamInfo> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    let updated = update_team_record(
        state,
        team,
        UpdateTeamInput {
            name: input.name,
            description: input.description,
        },
        input.policy_group_id,
    )
    .await?;
    build_admin_team_info(state, &updated).await
}

pub async fn archive_admin_team(state: &AppState, team_id: i64) -> Result<()> {
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    archive_team_record(state, team).await
}

pub async fn restore_admin_team(state: &AppState, team_id: i64) -> Result<AdminTeamInfo> {
    let team = team_repo::find_archived_by_id(&state.db, team_id).await?;
    let restored = restore_team_record(state, team).await?;
    build_admin_team_info(state, &restored).await
}

pub async fn cleanup_expired_archived_teams(state: &AppState) -> Result<u64> {
    let retention_days = load_team_archive_retention_days(state);
    let cutoff = Utc::now() - chrono::Duration::days(retention_days);
    let expired = team_repo::find_archived_before(&state.db, cutoff).await?;

    let mut deleted = 0u64;
    let ctx = audit_service::AuditContext::system();
    for team in expired {
        let team_id = team.id;
        let team_name = team.name.clone();
        let archived_at = team.archived_at;
        if let Err(err) = force_delete_archived_team(state, team).await {
            tracing::warn!(team_id, "failed to delete expired archived team: {err}");
            continue;
        }
        audit_service::log(
            state,
            &ctx,
            audit_service::AuditAction::TeamCleanupExpired,
            Some("team"),
            Some(team_id),
            Some(&team_name),
            audit_service::details(audit_service::TeamCleanupAuditDetails {
                archived_at,
                retention_days,
            }),
        )
        .await;
        deleted += 1;
    }

    Ok(deleted)
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
    let target_user =
        resolve_target_user(state, input.user_id, input.identifier.as_deref()).await?;
    if !target_user.status.is_active() {
        return Err(AsterError::validation_error(
            "cannot add a disabled user to a team",
        ));
    }

    let now = Utc::now();
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    team_repo::lock_active_by_id(&txn, team_id).await?;

    let actor_membership = team_member_repo::find_by_team_and_user(&txn, team_id, actor_user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    ensure_can_manage_team(actor_membership.role)?;
    if !actor_membership.role.is_owner() && input.role.is_owner() {
        return Err(AsterError::auth_forbidden(
            "only a team owner can assign owner role",
        ));
    }

    if team_member_repo::find_by_team_and_user(&txn, team_id, target_user.id)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(
            "user is already a team member",
        ));
    }

    let membership = team_member_repo::create(
        &txn,
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
    txn.commit().await.map_err(AsterError::from)?;

    Ok(build_team_member_info(membership, target_user))
}

pub async fn update_member_role(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    member_user_id: i64,
    role: TeamMemberRole,
) -> Result<TeamMemberInfo> {
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    team_repo::lock_active_by_id(&txn, team_id).await?;

    let actor_membership = team_member_repo::find_by_team_and_user(&txn, team_id, actor_user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    ensure_can_manage_team(actor_membership.role)?;

    let target_membership = team_member_repo::find_by_team_and_user(&txn, team_id, member_user_id)
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
        ensure_not_last_owner(&txn, team_id).await?;
    }
    if target_membership.role.can_manage_team() && !role.can_manage_team() {
        ensure_not_last_manager(&txn, team_id).await?;
    }

    let mut active = target_membership.clone().into_active_model();
    active.role = Set(role);
    active.updated_at = Set(Utc::now());
    let updated = team_member_repo::update(&txn, active).await?;
    let target_user = user_repo::find_by_id(&txn, member_user_id).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(build_team_member_info(updated, target_user))
}

pub async fn remove_member(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    member_user_id: i64,
) -> Result<()> {
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    team_repo::lock_active_by_id(&txn, team_id).await?;

    let actor_membership = team_member_repo::find_by_team_and_user(&txn, team_id, actor_user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    let target_membership = team_member_repo::find_by_team_and_user(&txn, team_id, member_user_id)
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
        ensure_not_last_owner(&txn, team_id).await?;
    }
    if target_membership.role.can_manage_team() {
        ensure_not_last_manager(&txn, team_id).await?;
    }

    team_member_repo::delete(&txn, target_membership.id).await?;
    txn.commit().await.map_err(AsterError::from)?;
    Ok(())
}
