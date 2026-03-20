use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};

use crate::config::AuthConfig;
use crate::db::repository::user_repo;
use crate::entities::user;
use crate::errors::{AsterError, Result};
use crate::types::{TokenType, UserRole, UserStatus};
use crate::utils::hash;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub user_id: i64,
    pub role: UserRole,
    pub token_type: TokenType,
    pub exp: usize,
}

/// 注册用户，返回用户信息（不含密码）
pub async fn register(
    db: &DatabaseConnection,
    username: &str,
    email: &str,
    password: &str,
    _jwt_secret: &str,
) -> Result<user::Model> {
    if user_repo::find_by_username(db, username).await?.is_some() {
        return Err(AsterError::validation_error("username already exists"));
    }
    if user_repo::find_by_email(db, email).await?.is_some() {
        return Err(AsterError::validation_error("email already exists"));
    }

    let password_hash = hash::hash_password(password)?;
    let now = Utc::now();

    // 第一个注册的用户自动成为 admin
    let is_first_user = user_repo::count_all(db).await? == 0;
    let role = if is_first_user {
        UserRole::Admin
    } else {
        UserRole::User
    };

    if is_first_user {
        tracing::info!("first user registered — granting admin role to '{username}'");
    }

    let model = user::ActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password_hash: Set(password_hash),
        role: Set(role),
        status: Set(UserStatus::Active),
        storage_used: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    user_repo::create(db, model).await
}

/// 登录，返回 (access_token, refresh_token)
pub async fn login(
    db: &DatabaseConnection,
    username: &str,
    password: &str,
    auth_config: &AuthConfig,
) -> Result<(String, String)> {
    let user = user_repo::find_by_username(db, username)
        .await?
        .ok_or_else(|| AsterError::auth_invalid_credentials("user not found"))?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }

    if !hash::verify_password(password, &user.password_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    let access = create_token(
        user.id,
        user.role,
        TokenType::Access,
        auth_config.access_token_ttl_secs,
        &auth_config.jwt_secret,
    )?;
    let refresh = create_token(
        user.id,
        user.role,
        TokenType::Refresh,
        auth_config.refresh_token_ttl_secs,
        &auth_config.jwt_secret,
    )?;

    Ok((access, refresh))
}

/// 用 refresh token 换 access token
pub fn refresh_token(refresh: &str, auth_config: &AuthConfig) -> Result<String> {
    let claims = verify_token(refresh, &auth_config.jwt_secret)?;
    if claims.token_type != TokenType::Refresh {
        return Err(AsterError::auth_token_invalid("not a refresh token"));
    }
    create_token(
        claims.user_id,
        claims.role,
        TokenType::Access,
        auth_config.access_token_ttl_secs,
        &auth_config.jwt_secret,
    )
}

fn create_token(
    user_id: i64,
    role: UserRole,
    token_type: TokenType,
    ttl_secs: u64,
    secret: &str,
) -> Result<String> {
    let exp = (Utc::now().timestamp() as u64 + ttl_secs) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id,
        role,
        token_type,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AsterError::internal_error(e.to_string()))
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
        _ => AsterError::auth_token_invalid(e.to_string()),
    })?;
    Ok(data.claims)
}
