use chrono::Utc;

use crate::db::repository::{share_repo, user_profile_repo, user_repo};
use crate::entities::share;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::profile_service;
use crate::utils::hash;

use super::models::{SharePublicInfo, SharePublicOwnerInfo};
use super::shared::{load_share_record, load_valid_share, resolve_share_name};

pub async fn get_share_info(state: &AppState, token: &str) -> Result<SharePublicInfo> {
    let db = &state.db;
    let share = load_valid_share(state, token).await?;
    tracing::debug!(share_id = share.id, "loading public share info");

    if let Err(error) = share_repo::increment_view_count(db, share.id).await {
        tracing::warn!(
            share_id = share.id,
            "failed to increment view count: {error}"
        );
    }

    let (name, share_type, mime_type, size) = resolve_share_name(db, &share).await?;
    let shared_by = resolve_share_owner_info(state, &share).await?;

    let is_expired = share.expires_at.is_some_and(|exp| exp < Utc::now());

    let info = SharePublicInfo {
        token: share.token,
        name,
        share_type,
        has_password: share.password.is_some(),
        expires_at: share.expires_at.map(|e| e.to_rfc3339()),
        is_expired,
        download_count: share.download_count,
        view_count: share.view_count,
        max_downloads: share.max_downloads,
        mime_type,
        size,
        shared_by,
    };
    tracing::debug!(
        share_id = share.id,
        has_password = info.has_password,
        is_expired = info.is_expired,
        download_count = info.download_count,
        view_count = info.view_count,
        "loaded public share info"
    );
    Ok(info)
}

fn resolve_share_owner_name(
    user: &crate::entities::user::Model,
    profile: Option<&crate::entities::user_profile::Model>,
) -> String {
    profile
        .and_then(|p| p.display_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| user.username.clone())
}

async fn resolve_share_owner_info(
    state: &AppState,
    share: &share::Model,
) -> Result<SharePublicOwnerInfo> {
    let user = user_repo::find_by_id(&state.db, share.user_id).await?;
    let profile = user_profile_repo::find_by_user_id(&state.db, share.user_id).await?;
    let gravatar_base_url = profile_service::resolve_gravatar_base_url(state);

    Ok(SharePublicOwnerInfo {
        name: resolve_share_owner_name(&user, profile.as_ref()),
        avatar: profile_service::build_share_public_avatar_info(
            &user,
            profile.as_ref(),
            &share.token,
            &gravatar_base_url,
        ),
    })
}

pub async fn get_share_avatar_bytes(state: &AppState, token: &str, size: u32) -> Result<Vec<u8>> {
    let share = load_valid_share(state, token).await?;
    profile_service::get_avatar_bytes(state, share.user_id, size).await
}

pub async fn verify_password(state: &AppState, token: &str, password: &str) -> Result<()> {
    let share = load_valid_share(state, token).await?;
    tracing::debug!(share_id = share.id, "verifying share password");

    let pw_hash = share
        .password
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("share has no password"))?;

    if !hash::verify_password(password, pw_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong share password"));
    }

    tracing::debug!(share_id = share.id, "verified share password");
    Ok(())
}

pub fn sign_share_cookie(token: &str, secret: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(format!("share_verified:{secret}:{token}").as_bytes());
    crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
}

pub fn verify_share_cookie(token: &str, cookie_value: &str, secret: &str) -> bool {
    let expected = sign_share_cookie(token, secret);
    if expected.len() != cookie_value.len() {
        return false;
    }
    expected
        .bytes()
        .zip(cookie_value.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub async fn check_share_password_cookie(
    state: &AppState,
    token: &str,
    cookie_value: Option<&str>,
) -> Result<()> {
    let share = load_share_record(state, token).await?;

    if share.password.is_some() {
        let value = cookie_value
            .ok_or_else(|| AsterError::share_password_required("password verification required"))?;

        if !verify_share_cookie(token, value, &state.config.auth.jwt_secret) {
            return Err(AsterError::share_password_required(
                "invalid verification cookie",
            ));
        }
    }
    Ok(())
}

pub struct PasswordVerified {
    pub cookie_signature: String,
}

pub async fn verify_password_and_sign(
    state: &AppState,
    token: &str,
    password: &str,
) -> Result<PasswordVerified> {
    verify_password(state, token, password).await?;
    Ok(PasswordVerified {
        cookie_signature: sign_share_cookie(token, &state.config.auth.jwt_secret),
    })
}
