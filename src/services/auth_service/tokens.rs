use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::entities::user;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::TokenType;

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
    let exp = (Utc::now().timestamp() as u64 + ttl_secs) as usize;
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
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
            AsterError::auth_token_expired("token expired")
        }
        _ => AsterError::auth_token_invalid("invalid token"),
    })?;
    Ok(data.claims)
}
