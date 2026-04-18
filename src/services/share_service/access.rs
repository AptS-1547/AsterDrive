//! 分享服务子模块：`access`。

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

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "share_secret_12345";

    #[test]
    fn sign_verify_share_cookie_roundtrip() {
        let token = "abc123xyz";
        let cookie = sign_share_cookie(token, SECRET);
        assert!(!cookie.is_empty());
        assert!(verify_share_cookie(token, &cookie, SECRET));
    }

    #[test]
    fn verify_share_cookie_rejects_wrong_token() {
        let token_a = "token_a";
        let token_b = "token_b";
        let cookie = sign_share_cookie(token_a, SECRET);
        assert!(!verify_share_cookie(token_b, &cookie, SECRET));
    }

    #[test]
    fn verify_share_cookie_rejects_wrong_secret() {
        let token = "token";
        let cookie = sign_share_cookie(token, SECRET);
        assert!(!verify_share_cookie(token, &cookie, "wrong_secret"));
    }

    #[test]
    fn verify_share_cookie_rejects_short_value() {
        let token = "token";
        // wrong length
        assert!(!verify_share_cookie(token, "short", SECRET));
    }

    #[test]
    fn resolve_share_owner_name_prefers_display_name() {
        let user = crate::entities::user::Model {
            id: 1,
            username: "alice".to_string(),
            email: "alice@test.com".to_string(),
            password_hash: String::new(),
            role: crate::types::UserRole::User,
            status: crate::types::UserStatus::Active,
            session_version: 0,
            email_verified_at: None,
            pending_email: None,
            storage_used: 0,
            storage_quota: 0,
            policy_group_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            config: None,
        };
        let profile = crate::entities::user_profile::Model {
            user_id: 1,
            display_name: Some("Alicia".to_string()),
            wopi_user_info: None,
            avatar_source: crate::types::AvatarSource::None,
            avatar_key: None,
            avatar_version: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let name = resolve_share_owner_name(&user, Some(&profile));
        assert_eq!(name, "Alicia");
    }

    #[test]
    fn resolve_share_owner_name_falls_back_to_username() {
        let user = crate::entities::user::Model {
            id: 1,
            username: "bob".to_string(),
            email: "bob@test.com".to_string(),
            password_hash: String::new(),
            role: crate::types::UserRole::User,
            status: crate::types::UserStatus::Active,
            session_version: 0,
            email_verified_at: None,
            pending_email: None,
            storage_used: 0,
            storage_quota: 0,
            policy_group_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            config: None,
        };
        let name = resolve_share_owner_name(&user, None);
        assert_eq!(name, "bob");
    }

    #[test]
    fn resolve_share_owner_name_skips_empty_display_name() {
        let user = crate::entities::user::Model {
            id: 1,
            username: "charlie".to_string(),
            email: "charlie@test.com".to_string(),
            password_hash: String::new(),
            role: crate::types::UserRole::User,
            status: crate::types::UserStatus::Active,
            session_version: 0,
            email_verified_at: None,
            pending_email: None,
            storage_used: 0,
            storage_quota: 0,
            policy_group_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            config: None,
        };
        let profile = crate::entities::user_profile::Model {
            user_id: 1,
            display_name: Some("   ".to_string()),
            wopi_user_info: None,
            avatar_source: crate::types::AvatarSource::None,
            avatar_key: None,
            avatar_version: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let name = resolve_share_owner_name(&user, Some(&profile));
        assert_eq!(name, "charlie");
    }
}
