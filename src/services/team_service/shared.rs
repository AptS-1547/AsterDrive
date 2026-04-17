//! 团队服务内部共用 helper。
//!
//! 这层主要负责：
//! - Team / TeamMember DTO 构建
//! - 常见权限断言
//! - “至少保留一个 owner/manager”这类跨入口共用约束

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{ConnectionTrait, DbErr, IntoActiveModel, Set, SqlErr, TransactionTrait};

use crate::config::operations;
use crate::db::repository::{policy_group_repo, team_member_repo, team_repo, user_repo};
use crate::entities::{team, team_member, user};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::TeamMemberRole;

use super::{
    AdminTeamInfo, CreateTeamInput, TeamInfo, TeamMemberInfo, TeamMemberListFilters,
    TeamMemberPage, UpdateTeamInput,
};

const MISSING_CREATOR_USERNAME: &str = "<deleted_user>";

fn map_team_member_create_db_err(err: DbErr) -> AsterError {
    if matches!(err.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        AsterError::validation_error("user is already a team member")
    } else {
        AsterError::from(err)
    }
}

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

pub(super) fn missing_creator_username(team: &team::Model) -> String {
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

pub(super) async fn build_team_info(
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

pub(super) fn build_team_info_with_metadata(
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

pub(super) async fn build_admin_team_info(
    state: &AppState,
    team: &team::Model,
) -> Result<AdminTeamInfo> {
    let creator_username = load_creator_username(state, team).await?;
    let member_count = team_member_repo::count_by_team(&state.db, team.id).await?;

    Ok(build_admin_team_info_with_metadata(
        team,
        creator_username,
        member_count,
    ))
}

pub(super) fn build_admin_team_info_with_metadata(
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

pub(super) fn build_team_member_info(
    membership: team_member::Model,
    user: user::Model,
) -> TeamMemberInfo {
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

fn build_team_member_page(
    items: Vec<TeamMemberInfo>,
    total: u64,
    limit: u64,
    offset: u64,
    owner_count: u64,
    manager_count: u64,
) -> TeamMemberPage {
    TeamMemberPage {
        items,
        total,
        limit,
        offset,
        owner_count,
        manager_count,
    }
}

pub(super) async fn load_team_member_page(
    state: &AppState,
    team_id: i64,
    filters: &TeamMemberListFilters,
    limit: u64,
    offset: u64,
) -> Result<TeamMemberPage> {
    // 成员列表页除了 items 之外，还顺手带回 owner/admin 计数，
    // 这样前端改角色时可以直接展示“至少保留一个管理员”的上下文信息。
    let effective_limit = limit.clamp(
        1,
        operations::team_member_list_max_limit(&state.runtime_config),
    );
    let (rows, total) = team_member_repo::list_page_by_team_with_user(
        &state.db,
        team_id,
        filters.role,
        filters.status,
        filters.keyword.as_deref(),
        effective_limit,
        offset,
    )
    .await?;
    let owner_count =
        team_member_repo::count_by_team_and_role(&state.db, team_id, TeamMemberRole::Owner).await?;
    let admin_count =
        team_member_repo::count_by_team_and_role(&state.db, team_id, TeamMemberRole::Admin).await?;

    Ok(build_team_member_page(
        rows.into_iter()
            .map(|(membership, user)| build_team_member_info(membership, user))
            .collect(),
        total,
        effective_limit,
        offset,
        owner_count,
        owner_count + admin_count,
    ))
}

pub(super) async fn resolve_target_user(
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

pub(super) async fn require_team_membership(
    state: &AppState,
    team_id: i64,
    user_id: i64,
) -> Result<(team::Model, team_member::Model)> {
    // 这里故意只接受 active team。
    // 对归档团队的访问需要走专门恢复 / admin 流程，避免普通团队 API 混入 archived 语义。
    let team = team_repo::find_active_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, user_id)
        .await?
        .ok_or_else(|| AsterError::auth_forbidden("not a member of this team"))?;
    Ok((team, membership))
}

pub(super) fn ensure_can_manage_team(role: TeamMemberRole) -> Result<()> {
    if !role.can_manage_team() {
        return Err(AsterError::auth_forbidden(
            "team owner or admin role is required",
        ));
    }
    Ok(())
}

pub(super) async fn ensure_not_last_owner<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<()> {
    // owner 是团队最终兜底权限，任何降级/移除操作都不能把数量减到 0。
    let owner_count =
        team_member_repo::count_by_team_and_role(db, team_id, TeamMemberRole::Owner).await?;
    if owner_count <= 1 {
        return Err(AsterError::validation_error(
            "team must keep at least one owner",
        ));
    }
    Ok(())
}

pub(super) async fn ensure_not_last_manager<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<()> {
    // manager = owner + admin。很多团队管理操作只要求“还有一个能管事的人”，
    // 因此这里的约束比 owner 更宽一点。
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

pub(super) async fn load_team_metadata(
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

pub(super) async fn ensure_assignable_policy_group(state: &AppState, group_id: i64) -> Result<()> {
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

pub(super) async fn resolve_required_policy_group_id(
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

pub(super) async fn create_team_record(
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

pub(super) async fn update_team_record(
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

pub(super) async fn archive_team_record(state: &AppState, team: team::Model) -> Result<()> {
    let mut active = team.into_active_model();
    let now = Utc::now();
    active.archived_at = Set(Some(now));
    active.updated_at = Set(now);
    team_repo::update(&state.db, active).await?;
    Ok(())
}

pub(super) async fn restore_team_record(
    state: &AppState,
    team: team::Model,
) -> Result<team::Model> {
    let mut active = team.into_active_model();
    let now = Utc::now();
    active.archived_at = Set(None);
    active.updated_at = Set(now);
    team_repo::update(&state.db, active).await
}

pub(super) fn map_member_create_error(err: DbErr) -> AsterError {
    map_team_member_create_db_err(err)
}
