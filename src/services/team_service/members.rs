use chrono::Utc;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set, TransactionTrait};

use crate::db::repository::{team_member_repo, team_repo, user_repo};
use crate::entities::team_member;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::TeamMemberRole;

use super::shared::{
    build_team_member_info, ensure_can_manage_team, ensure_not_last_manager, ensure_not_last_owner,
    load_team_member_page, map_member_create_error, require_team_membership, resolve_target_user,
};
use super::{AddTeamMemberInput, TeamMemberInfo, TeamMemberListFilters, TeamMemberPage};

pub async fn list_admin_members(
    state: &AppState,
    team_id: i64,
    filters: TeamMemberListFilters,
    limit: u64,
    offset: u64,
) -> Result<TeamMemberPage> {
    team_repo::find_by_id(&state.db, team_id).await?;
    load_team_member_page(state, team_id, &filters, limit, offset).await
}

pub async fn get_admin_member(
    state: &AppState,
    team_id: i64,
    member_user_id: i64,
) -> Result<TeamMemberInfo> {
    team_repo::find_by_id(&state.db, team_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, member_user_id)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("team member user #{member_user_id}"))
        })?;
    let user = user_repo::find_by_id(&state.db, member_user_id).await?;
    Ok(build_team_member_info(membership, user))
}

pub async fn add_admin_member(
    state: &AppState,
    team_id: i64,
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

    if team_member_repo::find_by_team_and_user(&txn, team_id, target_user.id)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(
            "user is already a team member",
        ));
    }

    let membership = team_member::ActiveModel {
        team_id: Set(team_id),
        user_id: Set(target_user.id),
        role: Set(input.role),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await
    .map_err(map_member_create_error)?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(build_team_member_info(membership, target_user))
}

pub async fn update_admin_member_role(
    state: &AppState,
    team_id: i64,
    member_user_id: i64,
    role: TeamMemberRole,
) -> Result<TeamMemberInfo> {
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    team_repo::lock_active_by_id(&txn, team_id).await?;

    let target_membership = team_member_repo::find_by_team_and_user(&txn, team_id, member_user_id)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("team member user #{member_user_id}"))
        })?;

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

pub async fn remove_admin_member(
    state: &AppState,
    team_id: i64,
    member_user_id: i64,
) -> Result<()> {
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    team_repo::lock_active_by_id(&txn, team_id).await?;

    let target_membership = team_member_repo::find_by_team_and_user(&txn, team_id, member_user_id)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("team member user #{member_user_id}"))
        })?;

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

pub async fn list_members(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    filters: TeamMemberListFilters,
    limit: u64,
    offset: u64,
) -> Result<TeamMemberPage> {
    require_team_membership(state, team_id, actor_user_id).await?;
    load_team_member_page(state, team_id, &filters, limit, offset).await
}

pub async fn get_member(
    state: &AppState,
    team_id: i64,
    actor_user_id: i64,
    member_user_id: i64,
) -> Result<TeamMemberInfo> {
    require_team_membership(state, team_id, actor_user_id).await?;
    let membership = team_member_repo::find_by_team_and_user(&state.db, team_id, member_user_id)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("team member user #{member_user_id}"))
        })?;
    let user = user_repo::find_by_id(&state.db, member_user_id).await?;
    Ok(build_team_member_info(membership, user))
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

    let membership = team_member::ActiveModel {
        team_id: Set(team_id),
        user_id: Set(target_user.id),
        role: Set(input.role),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await
    .map_err(map_member_create_error)?;
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
