use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, EntityTrait, IntoActiveModel, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::cache::CacheExt;
use crate::config::auth_runtime::{RuntimeAuthPolicy, RuntimeContactVerificationPolicy};
use crate::db::repository::{contact_verification_token_repo, user_repo};
use crate::entities::{contact_verification_token, user};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::mail_service;
use crate::types::{TokenType, UserRole, UserStatus, VerificationChannel, VerificationPurpose};
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

#[derive(Debug)]
pub struct ContactVerificationConfirmResult {
    pub purpose: VerificationPurpose,
    pub user_id: i64,
    pub target: String,
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

pub fn is_email_verified(user: &user::Model) -> bool {
    user.email_verified_at.is_some()
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

fn normalize_username(username: &str) -> Result<String> {
    let normalized = username.trim();
    validate_username(normalized)?;
    Ok(normalized.to_string())
}

fn normalize_email(email: &str) -> Result<String> {
    let normalized = email.trim();
    validate_email(normalized)?;
    Ok(normalized.to_string())
}

async fn ensure_email_available<C: ConnectionTrait>(
    db: &C,
    email: &str,
    exclude_user_id: Option<i64>,
) -> Result<()> {
    if let Some(existing) = user_repo::find_by_email(db, email).await?
        && Some(existing.id) != exclude_user_id
    {
        return Err(AsterError::validation_error("email already exists"));
    }

    if let Some(existing) = user_repo::find_by_pending_email(db, email).await?
        && Some(existing.id) != exclude_user_id
    {
        return Err(AsterError::validation_error("email already exists"));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_user_with_role<C: ConnectionTrait>(
    db: &C,
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
    role: UserRole,
    status: UserStatus,
    email_verified_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<user::Model> {
    let username = normalize_username(username)?;
    let email = normalize_email(email)?;
    validate_password(password)?;

    if user_repo::find_by_username(db, &username).await?.is_some() {
        return Err(AsterError::validation_error("username already exists"));
    }
    ensure_email_available(db, &email, None).await?;

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
        username: Set(username),
        email: Set(email),
        password_hash: Set(password_hash),
        role: Set(role),
        status: Set(status),
        session_version: Set(INITIAL_SESSION_VERSION),
        email_verified_at: Set(email_verified_at),
        pending_email: Set(None),
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

async fn issue_contact_verification_token<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    purpose: VerificationPurpose,
    target: &str,
    ttl_secs: u64,
) -> Result<String> {
    let now = Utc::now();
    let token = mail_service::build_verification_token();
    let token_hash = hash::sha256_hex(token.as_bytes());

    contact_verification_token_repo::delete_active_for_user(
        db,
        user_id,
        VerificationChannel::Email,
        purpose,
    )
    .await?;

    contact_verification_token_repo::create(
        db,
        contact_verification_token::ActiveModel {
            user_id: Set(user_id),
            channel: Set(VerificationChannel::Email),
            purpose: Set(purpose),
            target: Set(target.to_string()),
            token_hash: Set(token_hash),
            expires_at: Set(now + Duration::seconds(ttl_secs as i64)),
            consumed_at: Set(None),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    Ok(token)
}

async fn ensure_resend_allowed<C: ConnectionTrait>(
    state: &AppState,
    db: &C,
    user_id: i64,
    purpose: VerificationPurpose,
) -> Result<()> {
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let Some(latest) = contact_verification_token_repo::find_latest_active_for_user(
        db,
        user_id,
        VerificationChannel::Email,
        purpose,
    )
    .await?
    else {
        return Ok(());
    };

    let allowed_at = latest.created_at + Duration::seconds(policy.resend_cooldown_secs as i64);
    if allowed_at > Utc::now() {
        let remaining = (allowed_at - Utc::now()).num_seconds().max(1);
        return Err(AsterError::rate_limited(format!(
            "please wait {remaining} seconds before resending",
        )));
    }

    Ok(())
}

async fn cleanup_failed_registration(state: &AppState, user_id: i64) {
    if let Err(e) = user::Entity::delete_by_id(user_id).exec(&state.db).await {
        tracing::warn!(user_id, error = %e, "failed to cleanup user after registration mail error");
    }
}

async fn clear_pending_email_change(state: &AppState, user_id: i64) {
    match user_repo::find_by_id(&state.db, user_id).await {
        Ok(existing) => {
            let txn = match state.db.begin().await {
                Ok(txn) => txn,
                Err(e) => {
                    tracing::warn!(user_id, error = %e, "failed to begin pending email cleanup transaction");
                    return;
                }
            };
            let mut active = existing.into_active_model();
            active.pending_email = Set(None);
            active.updated_at = Set(Utc::now());
            if let Err(e) = active.update(&txn).await.map_err(AsterError::from) {
                tracing::warn!(user_id, error = %e, "failed to clear pending email after mail error");
                return;
            }
            if let Err(e) = contact_verification_token_repo::delete_active_for_user(
                &txn,
                user_id,
                VerificationChannel::Email,
                VerificationPurpose::ContactChange,
            )
            .await
            {
                tracing::warn!(user_id, error = %e, "failed to delete stale email change tokens after mail error");
                return;
            }
            if let Err(e) = txn.commit().await.map_err(AsterError::from) {
                tracing::warn!(user_id, error = %e, "failed to commit pending email cleanup transaction");
            }
        }
        Err(e) => {
            tracing::warn!(user_id, error = %e, "failed to load user for pending email cleanup");
        }
    }
}

pub async fn create_user_by_admin(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<user::Model> {
    create_user_with_role(
        &state.db,
        state,
        username,
        email,
        password,
        UserRole::User,
        UserStatus::Active,
        Some(Utc::now()),
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
    if is_first_user {
        tracing::info!("first user registered — granting admin role to '{username}'");
        return create_user_with_role(
            &state.db,
            state,
            username,
            email,
            password,
            UserRole::Admin,
            UserStatus::Active,
            Some(Utc::now()),
        )
        .await;
    }

    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let user = create_user_with_role(
        &txn,
        state,
        username,
        email,
        password,
        UserRole::User,
        UserStatus::Active,
        None,
    )
    .await?;
    let token = issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::RegisterActivation,
        &user.email,
        policy.register_activation_ttl_secs,
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    if let Err(error) =
        mail_service::send_register_activation(state, &user.username, &user.email, &token).await
    {
        cleanup_failed_registration(state, user.id).await;
        return Err(error);
    }

    Ok(user)
}

pub async fn resend_register_activation(state: &AppState, identifier: &str) -> Result<user::Model> {
    let user = find_user_by_identifier(&state.db, identifier)
        .await?
        .ok_or_else(|| AsterError::record_not_found("user not found"))?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if is_email_verified(&user) {
        return Err(AsterError::validation_error("account already activated"));
    }

    ensure_resend_allowed(
        state,
        &state.db,
        user.id,
        VerificationPurpose::RegisterActivation,
    )
    .await?;
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let token = issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::RegisterActivation,
        &user.email,
        policy.register_activation_ttl_secs,
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    mail_service::send_register_activation(state, &user.username, &user.email, &token).await?;
    Ok(user)
}

pub async fn request_email_change(
    state: &AppState,
    user_id: i64,
    new_email: &str,
) -> Result<user::Model> {
    let normalized_email = normalize_email(new_email)?;
    let existing = user_repo::find_by_id(&state.db, user_id).await?;

    if !existing.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if !is_email_verified(&existing) {
        return Err(AsterError::auth_pending_activation(
            "account must be activated before changing email",
        ));
    }
    if existing.email == normalized_email {
        return Err(AsterError::validation_error(
            "new email must be different from current email",
        ));
    }

    ensure_email_available(&state.db, &normalized_email, Some(existing.id)).await?;
    if existing.pending_email.as_deref() == Some(normalized_email.as_str()) {
        ensure_resend_allowed(
            state,
            &state.db,
            existing.id,
            VerificationPurpose::ContactChange,
        )
        .await?;
    }

    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let mut active = existing.into_active_model();
    active.pending_email = Set(Some(normalized_email.clone()));
    active.updated_at = Set(Utc::now());
    let updated = active.update(&txn).await.map_err(AsterError::from)?;
    let token = issue_contact_verification_token(
        &txn,
        updated.id,
        VerificationPurpose::ContactChange,
        &normalized_email,
        policy.contact_change_ttl_secs,
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    if let Err(error) = mail_service::send_contact_change_confirmation(
        state,
        &updated.username,
        &normalized_email,
        &token,
    )
    .await
    {
        clear_pending_email_change(state, updated.id).await;
        return Err(error);
    }

    Ok(updated)
}

pub async fn resend_email_change(state: &AppState, user_id: i64) -> Result<user::Model> {
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let pending_email = user
        .pending_email
        .clone()
        .ok_or_else(|| AsterError::validation_error("no pending email change request"))?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if !is_email_verified(&user) {
        return Err(AsterError::auth_pending_activation(
            "account must be activated before changing email",
        ));
    }

    ensure_email_available(&state.db, &pending_email, Some(user.id)).await?;
    ensure_resend_allowed(
        state,
        &state.db,
        user.id,
        VerificationPurpose::ContactChange,
    )
    .await?;
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let token = issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::ContactChange,
        &pending_email,
        policy.contact_change_ttl_secs,
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    mail_service::send_contact_change_confirmation(state, &user.username, &pending_email, &token)
        .await?;
    Ok(user)
}

pub async fn confirm_contact_verification(
    state: &AppState,
    token: &str,
) -> Result<ContactVerificationConfirmResult> {
    let token_hash = hash::sha256_hex(token.as_bytes());
    let record = contact_verification_token_repo::find_by_token_hash(&state.db, &token_hash)
        .await?
        .ok_or_else(|| {
            AsterError::contact_verification_invalid("contact verification link is invalid")
        })?;

    if record.consumed_at.is_some() {
        return Err(AsterError::contact_verification_invalid(
            "contact verification link has already been used",
        ));
    }
    if record.expires_at <= Utc::now() {
        return Err(AsterError::contact_verification_expired(
            "contact verification link has expired",
        ));
    }

    let target = record.target.clone();
    let purpose = record.purpose;
    let user_id = record.user_id;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing_user = user_repo::find_by_id(&txn, user_id).await?;
    if !existing_user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    let username = existing_user.username.clone();
    let previous_email = (purpose == VerificationPurpose::ContactChange
        && existing_user.email != target)
        .then(|| existing_user.email.clone());

    let now = Utc::now();
    match purpose {
        VerificationPurpose::RegisterActivation => {
            if existing_user.email != target {
                return Err(AsterError::contact_verification_invalid(
                    "contact verification target mismatch",
                ));
            }

            if !is_email_verified(&existing_user) {
                let mut active = existing_user.into_active_model();
                active.email_verified_at = Set(Some(now));
                active.updated_at = Set(now);
                active.update(&txn).await.map_err(AsterError::from)?;
            }
        }
        VerificationPurpose::ContactChange => {
            if existing_user.email != target
                && existing_user.pending_email.as_deref() != Some(target.as_str())
            {
                return Err(AsterError::contact_verification_invalid(
                    "contact change request no longer exists",
                ));
            }

            ensure_email_available(&txn, &target, Some(existing_user.id)).await?;

            if existing_user.email != target {
                let mut active = existing_user.into_active_model();
                active.email = Set(target.clone());
                active.pending_email = Set(None);
                active.email_verified_at = Set(Some(now));
                active.updated_at = Set(now);
                active.update(&txn).await.map_err(AsterError::from)?;
            }
        }
    }

    contact_verification_token_repo::mark_consumed(&txn, record).await?;
    txn.commit().await.map_err(AsterError::from)?;

    if let Some(previous_email) = previous_email.as_deref()
        && let Err(error) =
            mail_service::send_contact_change_notice(state, &username, previous_email, &target)
                .await
    {
        tracing::warn!(
            user_id,
            previous_email,
            new_email = %target,
            error = %error,
            "failed to send email change notice to previous address"
        );
    }

    Ok(ContactVerificationConfirmResult {
        purpose,
        user_id,
        target,
    })
}

/// 检查标识符（邮箱或用户名）是否存在，以及系统是否有用户
pub async fn check_identifier(state: &AppState, identifier: &str) -> Result<(bool, bool)> {
    let db = &state.db;
    let has_users = user_repo::count_all(db).await? > 0;
    let exists = find_user_by_identifier(db, identifier).await?.is_some();
    Ok((exists, has_users))
}

/// 按标识符查找用户（支持邮箱或用户名）
async fn find_user_by_identifier<C: ConnectionTrait>(
    db: &C,
    identifier: &str,
) -> Result<Option<crate::entities::user::Model>> {
    let normalized = identifier.trim();
    if normalized.contains('@') {
        user_repo::find_by_email(db, normalized).await
    } else {
        user_repo::find_by_username(db, normalized).await
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

pub fn issue_tokens_for_user(state: &AppState, user: &user::Model) -> Result<(String, String)> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    issue_tokens(
        user.id,
        user.session_version,
        &state.config.auth.jwt_secret,
        auth_policy,
    )
}

/// 登录，返回 tokens + user_id
/// identifier 支持邮箱或用户名
pub async fn login(state: &AppState, identifier: &str, password: &str) -> Result<LoginResult> {
    let db = &state.db;
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);

    let user = find_user_by_identifier(db, identifier)
        .await?
        .ok_or_else(|| AsterError::auth_invalid_credentials("user not found"))?;

    if !user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if !is_email_verified(&user) {
        return Err(AsterError::auth_pending_activation(
            "account pending activation",
        ));
    }

    if !hash::verify_password(password, &user.password_hash)? {
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    let (access, refresh) = issue_tokens(
        user.id,
        user.session_version,
        &state.config.auth.jwt_secret,
        auth_policy,
    )?;

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

pub async fn cleanup_expired_contact_verification_tokens(state: &AppState) -> Result<u64> {
    contact_verification_token_repo::delete_expired(&state.db).await
}

/// 用 refresh token 换 access token
pub async fn refresh_token(state: &AppState, refresh: &str) -> Result<String> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let (claims, snapshot) = authenticate_refresh_token(state, refresh).await?;
    create_token(
        claims.user_id,
        snapshot.session_version,
        TokenType::Access,
        auth_policy.access_token_ttl_secs,
        &state.config.auth.jwt_secret,
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
