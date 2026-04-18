//! 认证服务子模块：`tokens`。

use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::entities::user;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::TokenType;
use crate::utils::numbers::{i64_to_u64, u64_to_usize};

use super::session::get_auth_snapshot;
use super::{AuthSnapshot, Claims};

fn ensure_token_type(claims: &Claims, expected: TokenType) -> Result<()> {
    if claims.token_type != expected {
        return Err(AsterError::auth_token_invalid(format!(
            "not an {} token",
            expected.as_str()
        )));
    }

    Ok(())
}

fn ensure_session_current(claims: &Claims, snapshot: AuthSnapshot) -> Result<()> {
    if claims.session_version != snapshot.session_version {
        return Err(AsterError::auth_token_invalid("session revoked"));
    }

    Ok(())
}

async fn authenticate_token(
    state: &AppState,
    token: &str,
    expected_type: TokenType,
) -> Result<(Claims, AuthSnapshot)> {
    tracing::debug!(
        expected_type = expected_type.as_str(),
        "authenticating token"
    );
    let claims = verify_token(token, &state.config.auth.jwt_secret)?;
    ensure_token_type(&claims, expected_type)?;

    let snapshot = get_auth_snapshot(state, claims.user_id).await?;
    if !snapshot.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    ensure_session_current(&claims, snapshot)?;

    tracing::debug!(
        user_id = claims.user_id,
        expected_type = expected_type.as_str(),
        session_version = snapshot.session_version,
        "authenticated token"
    );

    Ok((claims, snapshot))
}

pub async fn authenticate_access_token(
    state: &AppState,
    token: &str,
) -> Result<(Claims, AuthSnapshot)> {
    authenticate_token(state, token, TokenType::Access).await
}

pub async fn authenticate_refresh_token(
    state: &AppState,
    token: &str,
) -> Result<(Claims, AuthSnapshot)> {
    authenticate_token(state, token, TokenType::Refresh).await
}

fn issue_tokens(
    user_id: i64,
    session_version: i64,
    jwt_secret: &str,
    auth_policy: RuntimeAuthPolicy,
) -> Result<(String, String)> {
    let access = create_token(
        user_id,
        session_version,
        TokenType::Access,
        auth_policy.access_token_ttl_secs,
        jwt_secret,
    )?;
    let refresh = create_token(
        user_id,
        session_version,
        TokenType::Refresh,
        auth_policy.refresh_token_ttl_secs,
        jwt_secret,
    )?;
    Ok((access, refresh))
}

pub fn issue_tokens_for_session(
    state: &AppState,
    user_id: i64,
    session_version: i64,
) -> Result<(String, String)> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    issue_tokens(
        user_id,
        session_version,
        &state.config.auth.jwt_secret,
        auth_policy,
    )
}

pub fn issue_tokens_for_user(state: &AppState, user: &user::Model) -> Result<(String, String)> {
    issue_tokens_for_session(state, user.id, user.session_version)
}

pub async fn refresh_token(state: &AppState, refresh: &str) -> Result<String> {
    tracing::debug!("refreshing access token");
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let (claims, snapshot) = authenticate_refresh_token(state, refresh).await?;
    let token = create_token(
        claims.user_id,
        snapshot.session_version,
        TokenType::Access,
        auth_policy.access_token_ttl_secs,
        &state.config.auth.jwt_secret,
    )?;
    tracing::debug!(
        user_id = claims.user_id,
        session_version = snapshot.session_version,
        "refreshed access token"
    );
    Ok(token)
}

fn create_token(
    user_id: i64,
    session_version: i64,
    token_type: TokenType,
    ttl_secs: u64,
    secret: &str,
) -> Result<String> {
    let now_secs = i64_to_u64(Utc::now().timestamp(), "jwt issued_at unix timestamp")?;
    let exp_secs = now_secs.checked_add(ttl_secs).ok_or_else(|| {
        AsterError::internal_error(format!("jwt exp overflow: {now_secs} + {ttl_secs}"))
    })?;
    let exp = u64_to_usize(exp_secs, "jwt exp")?;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id,
        session_version,
        token_type,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_aster_err(AsterError::internal_error)
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims> {
    let data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(error) => {
            return Err(match error.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    AsterError::auth_token_expired("token expired")
                }
                _ => AsterError::auth_token_invalid("invalid token"),
            });
        }
    };
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TokenType;

    const SECRET: &str = "test_secret_32bytes_xxxxxxxxxxxxxxxxx";

    fn make_token(token_type: TokenType, ttl_secs: u64, secret: &str) -> String {
        create_token(1, 1, token_type, ttl_secs, secret).unwrap()
    }

    #[test]
    fn verify_access_token_roundtrip() {
        let token = make_token(TokenType::Access, 3600, SECRET);
        let claims = verify_token(&token, SECRET).unwrap();
        assert_eq!(claims.user_id, 1);
        assert_eq!(claims.session_version, 1);
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn verify_refresh_token_roundtrip() {
        let token = make_token(TokenType::Refresh, 86400, SECRET);
        let claims = verify_token(&token, SECRET).unwrap();
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn verify_token_rejects_wrong_secret() {
        let token = make_token(TokenType::Access, 3600, SECRET);
        let err = verify_token(&token, "wrong_secret").unwrap_err();
        // jsonwebtoken 的 InvalidSignature 归类到 "invalid token"
        assert_eq!(err.code(), "E012"); // AuthTokenInvalid
    }

    #[test]
    fn ensure_token_type_access_rejects_refresh() {
        let token = make_token(TokenType::Refresh, 3600, SECRET);
        let claims = verify_token(&token, SECRET).unwrap();
        let err = ensure_token_type(&claims, TokenType::Access).unwrap_err();
        assert_eq!(err.code(), "E012");
    }

    #[test]
    fn ensure_session_current_rejects_stale_version() {
        let claims = Claims {
            sub: "1".to_string(),
            user_id: 1,
            session_version: 1,
            token_type: TokenType::Access,
            exp: usize::MAX, // 永不过期，只测 version
        };
        let snapshot = crate::services::auth_service::AuthSnapshot {
            session_version: 2,
            status: crate::types::UserStatus::Active,
            role: crate::types::UserRole::User,
        };
        let err = ensure_session_current(&claims, snapshot).unwrap_err();
        assert_eq!(err.code(), "E012");
    }

    #[test]
    fn ensure_session_current_accepts_matching_version() {
        let claims = Claims {
            sub: "1".to_string(),
            user_id: 1,
            session_version: 1,
            token_type: TokenType::Access,
            exp: usize::MAX,
        };
        let snapshot = crate::services::auth_service::AuthSnapshot {
            session_version: 1,
            status: crate::types::UserStatus::Active,
            role: crate::types::UserRole::User,
        };
        assert!(ensure_session_current(&claims, snapshot).is_ok());
    }
}
