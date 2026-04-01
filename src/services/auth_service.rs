use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde::{Deserialize, Serialize};

use crate::cache::CacheExt;
use crate::config::AuthConfig;
use crate::db::repository::user_repo;
use crate::entities::user;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::{TokenType, UserRole, UserStatus};
use crate::utils::hash;

pub const AUTH_SNAPSHOT_TTL: u64 = 30; // 秒
const INITIAL_SESSION_VERSION: i64 = 1;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub user_id: i64,
    #[serde(default = "default_session_version")]
    pub session_version: i64,
    pub token_type: TokenType,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct AuthSnapshot {
    pub status: UserStatus,
    pub role: UserRole,
    pub session_version: i64,
}

impl AuthSnapshot {
    fn from_user(user: &user::Model) -> Self {
        Self {
            status: user.status,
            role: user.role,
            session_version: user.session_version,
        }
    }
}

fn default_session_version() -> i64 {
    0
}

fn auth_snapshot_cache_key(user_id: i64) -> String {
    format!("auth_snapshot:{user_id}")
}

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

pub async fn get_auth_snapshot(state: &AppState, user_id: i64) -> Result<AuthSnapshot> {
    let cache_key = auth_snapshot_cache_key(user_id);
    if let Some(snapshot) = state.cache.get(&cache_key).await {
        return Ok(snapshot);
    }

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let snapshot = AuthSnapshot::from_user(&user);
    state
        .cache
        .set(&cache_key, &snapshot, Some(AUTH_SNAPSHOT_TTL))
        .await;
    Ok(snapshot)
}

pub async fn invalidate_auth_snapshot_cache(state: &AppState, user_id: i64) {
    state.cache.delete(&auth_snapshot_cache_key(user_id)).await;
}

async fn authenticate_token(
    state: &AppState,
    token: &str,
    expected_type: TokenType,
) -> Result<(Claims, AuthSnapshot)> {
    let claims = verify_token(token, &state.config.auth.jwt_secret)?;
    ensure_token_type(&claims, expected_type)?;

    let snapshot = get_auth_snapshot(state, claims.user_id).await?;
    if !snapshot.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    ensure_session_current(&claims, snapshot)?;

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

pub async fn revoke_user_sessions(state: &AppState, user_id: i64) -> Result<user::Model> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let next_session_version = user.session_version.saturating_add(1);
    let mut active = user.into_active_model();
    active.session_version = Set(next_session_version);
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await.map_err(AsterError::from)?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    Ok(updated)
}

// ── 输入校验 ──────────────────────────────────────────────────

fn validate_username(username: &str) -> Result<()> {
    let len = username.len();
    if len < 4 {
        return Err(AsterError::validation_error(
            "username must be at least 4 characters",
        ));
    }
    if len > 16 {
        return Err(AsterError::validation_error(
            "username must be at most 16 characters",
        ));
    }
    // 只允许字母、数字、下划线、连字符
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AsterError::validation_error(
            "username may only contain letters, numbers, underscores and hyphens",
        ));
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<()> {
    if email.len() > 254 {
        return Err(AsterError::validation_error("email is too long"));
    }
    // 基础格式校验：有且仅有一个 @，@ 前后非空，@ 后有点
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(AsterError::validation_error("invalid email format"));
    }
    if !parts[1].contains('.') {
        return Err(AsterError::validation_error("invalid email format"));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<()> {
    if password.len() < 6 {
        return Err(AsterError::validation_error(
            "password must be at least 6 characters",
        ));
    }
    if password.len() > 128 {
        return Err(AsterError::validation_error(
            "password must be at most 128 characters",
        ));
    }
    Ok(())
}

async fn create_user_with_role(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
    role: UserRole,
    status: UserStatus,
) -> Result<user::Model> {
    let db = &state.db;

    validate_username(username)?;
    validate_email(email)?;
    validate_password(password)?;

    if user_repo::find_by_username(db, username).await?.is_some() {
        return Err(AsterError::validation_error("username already exists"));
    }
    if user_repo::find_by_email(db, email).await?.is_some() {
        return Err(AsterError::validation_error("email already exists"));
    }

    let password_hash = hash::hash_password(password)?;
    let now = Utc::now();

    let default_quota = state
        .runtime_config
        .get_i64("default_storage_quota")
        .unwrap_or_else(|| {
            if let Some(raw) = state.runtime_config.get("default_storage_quota") {
                tracing::warn!("invalid default_storage_quota value '{}', using 0", raw);
            }
            0
        });
    let default_policy_group_id = state
        .policy_snapshot
        .system_default_policy_group()
        .map(|group| group.id)
        .ok_or_else(|| {
            AsterError::storage_policy_not_found(
                "no system default storage policy group configured",
            )
        })?;

    let model = user::ActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password_hash: Set(password_hash),
        role: Set(role),
        status: Set(status),
        session_version: Set(INITIAL_SESSION_VERSION),
        storage_used: Set(0),
        storage_quota: Set(default_quota),
        policy_group_id: Set(Some(default_policy_group_id)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let user = user_repo::create(db, model).await?;

    if let Some(policy_group_id) = user.policy_group_id {
        state
            .policy_snapshot
            .set_user_policy_group(user.id, policy_group_id);
    }

    Ok(user)
}

pub async fn create_user_by_admin(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<user::Model> {
    create_user_with_role(
        state,
        username,
        email,
        password,
        UserRole::User,
        UserStatus::Active,
    )
    .await
}

/// 注册用户，返回用户信息（不含密码）
pub async fn register(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<user::Model> {
    let is_first_user = user_repo::count_all(&state.db).await? == 0;
    let role = if is_first_user {
        UserRole::Admin
    } else {
        UserRole::User
    };

    if is_first_user {
        tracing::info!("first user registered — granting admin role to '{username}'");
    }

    create_user_with_role(state, username, email, password, role, UserStatus::Active).await
}

/// 检查标识符（邮箱或用户名）是否存在，以及系统是否有用户
pub async fn check_identifier(state: &AppState, identifier: &str) -> Result<(bool, bool)> {
    let db = &state.db;
    let has_users = user_repo::count_all(db).await? > 0;
    let exists = find_user_by_identifier(db, identifier).await?.is_some();
    Ok((exists, has_users))
}

/// 按标识符查找用户（支持邮箱或用户名）
async fn find_user_by_identifier(
    db: &sea_orm::DatabaseConnection,
    identifier: &str,
) -> Result<Option<crate::entities::user::Model>> {
    if identifier.contains('@') {
        user_repo::find_by_email(db, identifier).await
    } else {
        user_repo::find_by_username(db, identifier).await
    }
}

/// 首次设置：仅在无用户时创建管理员
pub async fn setup(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<crate::entities::user::Model> {
    let db = &state.db;
    if user_repo::count_all(db).await? > 0 {
        return Err(AsterError::validation_error("system already initialized"));
    }
    register(state, username, email, password).await
}

/// 登录结果：access/refresh tokens + user_id（用于审计）
pub struct LoginResult {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: i64,
}

fn issue_tokens(
    user_id: i64,
    session_version: i64,
    auth_config: &AuthConfig,
) -> Result<(String, String)> {
    let access = create_token(
        user_id,
        session_version,
        TokenType::Access,
        auth_config.access_token_ttl_secs,
        &auth_config.jwt_secret,
    )?;
    let refresh = create_token(
        user_id,
        session_version,
        TokenType::Refresh,
        auth_config.refresh_token_ttl_secs,
        &auth_config.jwt_secret,
    )?;
    Ok((access, refresh))
}

pub fn issue_tokens_for_user(
    user: &user::Model,
    auth_config: &AuthConfig,
) -> Result<(String, String)> {
    issue_tokens(user.id, user.session_version, auth_config)
}

/// 登录，返回 tokens + user_id
/// identifier 支持邮箱或用户名
pub async fn login(state: &AppState, identifier: &str, password: &str) -> Result<LoginResult> {
    let db = &state.db;
    let auth_config = &state.config.auth;

    let user = find_user_by_identifier(db, identifier)
        .await?
        .ok_or_else(|| AsterError::auth_invalid_credentials("user not found"))?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }

    if !hash::verify_password(password, &user.password_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    let (access, refresh) = issue_tokens(user.id, user.session_version, auth_config)?;

    Ok(LoginResult {
        access_token: access,
        refresh_token: refresh,
        user_id: user.id,
    })
}

pub async fn change_password(
    state: &AppState,
    user_id: i64,
    current_password: &str,
    new_password: &str,
) -> Result<user::Model> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }

    if !hash::verify_password(current_password, &user.password_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    set_password(state, user.id, new_password).await
}

pub async fn set_password(
    state: &AppState,
    user_id: i64,
    new_password: &str,
) -> Result<user::Model> {
    validate_password(new_password)?;

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let next_session_version = user.session_version.saturating_add(1);
    let mut active = user.into_active_model();
    active.password_hash = Set(hash::hash_password(new_password)?);
    active.session_version = Set(next_session_version);
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await.map_err(AsterError::from)?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    Ok(updated)
}

/// 用 refresh token 换 access token
pub async fn refresh_token(state: &AppState, refresh: &str) -> Result<String> {
    let auth_config = &state.config.auth;
    let (claims, snapshot) = authenticate_refresh_token(state, refresh).await?;
    create_token(
        claims.user_id,
        snapshot.session_version,
        TokenType::Access,
        auth_config.access_token_ttl_secs,
        &auth_config.jwt_secret,
    )
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
