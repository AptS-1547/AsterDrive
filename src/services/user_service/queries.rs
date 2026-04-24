use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::user_repo;
use crate::entities::user;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::{auth_service, profile_service};
use crate::types::{UserRole, UserStatus};

use super::models::{MeResponse, UserInfo, user_core};
use super::preferences::parse_preferences;

pub async fn to_user_info(
    state: &PrimaryAppState,
    user: &user::Model,
    audience: profile_service::AvatarAudience,
) -> Result<UserInfo> {
    let core = user_core(user);
    Ok(UserInfo {
        id: core.id,
        username: core.username,
        email: core.email,
        email_verified: core.email_verified,
        pending_email: core.pending_email,
        role: core.role,
        status: core.status,
        storage_used: core.storage_used,
        storage_quota: core.storage_quota,
        policy_group_id: core.policy_group_id,
        created_at: core.created_at,
        updated_at: core.updated_at,
        profile: profile_service::get_profile_info(state, user, audience).await?,
    })
}

pub async fn to_user_infos(
    state: &PrimaryAppState,
    users: Vec<user::Model>,
    audience: profile_service::AvatarAudience,
) -> Result<Vec<UserInfo>> {
    let profile_map = profile_service::get_profile_info_map(state, &users, audience).await?;
    let gravatar_base_url = profile_service::resolve_gravatar_base_url(state);

    Ok(users
        .into_iter()
        .map(|user| UserInfo {
            id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            email_verified: auth_service::is_email_verified(&user),
            pending_email: user.pending_email.clone(),
            role: user.role,
            status: user.status,
            storage_used: user.storage_used,
            storage_quota: user.storage_quota,
            policy_group_id: user.policy_group_id,
            created_at: user.created_at,
            updated_at: user.updated_at,
            profile: profile_map.get(&user.id).cloned().unwrap_or_else(|| {
                profile_service::build_profile_info(&user, None, audience, &gravatar_base_url)
            }),
        })
        .collect())
}

pub async fn get_me(
    state: &PrimaryAppState,
    user_id: i64,
    access_token_expires_at: i64,
) -> Result<MeResponse> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let prefs = parse_preferences(&user);
    let core = user_core(&user);
    Ok(MeResponse {
        id: core.id,
        username: core.username,
        email: core.email,
        email_verified: core.email_verified,
        pending_email: core.pending_email,
        role: core.role,
        status: core.status,
        storage_used: core.storage_used,
        storage_quota: core.storage_quota,
        policy_group_id: core.policy_group_id,
        access_token_expires_at,
        created_at: core.created_at,
        updated_at: core.updated_at,
        preferences: prefs,
        profile: profile_service::get_profile_info(
            state,
            &user,
            profile_service::AvatarAudience::SelfUser,
        )
        .await?,
    })
}

pub async fn get_self_info(state: &PrimaryAppState, user_id: i64) -> Result<UserInfo> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    to_user_info(state, &user, profile_service::AvatarAudience::SelfUser).await
}

pub async fn list_paginated(
    state: &PrimaryAppState,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
    role: Option<UserRole>,
    status: Option<UserStatus>,
) -> Result<OffsetPage<UserInfo>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        user_repo::find_paginated(&state.db, limit, offset, keyword, role, status).await
    })
    .await?;

    Ok(OffsetPage::new(
        to_user_infos(
            state,
            page.items,
            profile_service::AvatarAudience::AdminUser,
        )
        .await?,
        page.total,
        page.limit,
        page.offset,
    ))
}

pub async fn get(state: &PrimaryAppState, id: i64) -> Result<UserInfo> {
    let user = user_repo::find_by_id(&state.db, id).await?;
    to_user_info(state, &user, profile_service::AvatarAudience::AdminUser).await
}
