use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ConnectionTrait, DbErr, IntoActiveModel, Set, SqlErr,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::cache::CacheExt;
use crate::config::auth_runtime::{RuntimeAuthPolicy, RuntimeContactVerificationPolicy};
use crate::db::repository::{contact_verification_token_repo, user_repo};
use crate::entities::{contact_verification_token, user};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{mail_outbox_service, mail_service, mail_template::MailTemplatePayload};
use crate::types::{TokenType, UserRole, UserStatus, VerificationChannel, VerificationPurpose};
use crate::utils::hash;

pub const AUTH_SNAPSHOT_TTL: u64 = 30; // 秒
const INITIAL_SESSION_VERSION: i64 = 1;
const ACTIVE_VERIFICATION_REQUEST_MESSAGE: &str = "a verification request is already active";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAuditInfo {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserInfo {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub session_version: i64,
    pub email_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pending_email: Option<String>,
    pub storage_used: i64,
    pub storage_quota: i64,
    pub policy_group_id: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub config: Option<String>,
}

impl From<user::Model> for AuthUserInfo {
    fn from(model: user::Model) -> Self {
        Self {
            id: model.id,
            username: model.username,
            email: model.email,
            role: model.role,
            status: model.status,
            session_version: model.session_version,
            email_verified_at: model.email_verified_at,
            pending_email: model.pending_email,
            storage_used: model.storage_used,
            storage_quota: model.storage_quota,
            policy_group_id: model.policy_group_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
            config: model.config,
        }
    }
}

impl From<AuthUserInfo> for user::ActiveModel {
    fn from(info: AuthUserInfo) -> Self {
        Self {
            id: Set(info.id),
            username: Set(info.username),
            email: Set(info.email),
            password_hash: ActiveValue::NotSet,
            role: Set(info.role),
            status: Set(info.status),
            session_version: Set(info.session_version),
            email_verified_at: Set(info.email_verified_at),
            pending_email: Set(info.pending_email),
            storage_used: Set(info.storage_used),
            storage_quota: Set(info.storage_quota),
            policy_group_id: Set(info.policy_group_id),
            created_at: Set(info.created_at),
            updated_at: Set(info.updated_at),
            config: Set(info.config),
        }
    }
}

#[derive(Debug)]
pub struct PasswordResetRequestResult {
    pub user: Option<UserAuditInfo>,
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

fn user_audit_info(user: &user::Model) -> UserAuditInfo {
    UserAuditInfo {
        id: user.id,
        username: user.username.clone(),
    }
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
        tracing::debug!(user_id, "auth snapshot cache hit");
        return Ok(snapshot);
    }

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let snapshot = AuthSnapshot::from_user(&user);
    state
        .cache
        .set(&cache_key, &snapshot, Some(AUTH_SNAPSHOT_TTL))
        .await;
    tracing::debug!(user_id, "auth snapshot cache miss");
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

pub async fn revoke_user_sessions(state: &AppState, user_id: i64) -> Result<UserAuditInfo> {
    tracing::debug!(user_id, "revoking user sessions");
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let next_session_version = user.session_version.saturating_add(1);
    let mut active = user.into_active_model();
    active.session_version = Set(next_session_version);
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await.map_err(AsterError::from)?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    tracing::debug!(
        user_id = updated.id,
        session_version = updated.session_version,
        "revoked user sessions"
    );
    Ok(user_audit_info(&updated))
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
    if password.len() < 8 {
        return Err(AsterError::validation_error(
            "password must be at least 8 characters",
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

fn is_unique_conflict_db_err(err: &DbErr) -> bool {
    matches!(err.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
}

async fn map_user_create_db_err<C: ConnectionTrait>(
    db: &C,
    err: DbErr,
    username: &str,
    email: &str,
) -> AsterError {
    if !is_unique_conflict_db_err(&err) {
        return AsterError::from(err);
    }

    if user_repo::find_by_username(db, username)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return AsterError::validation_error("username already exists");
    }

    if user_repo::find_by_email(db, email)
        .await
        .ok()
        .flatten()
        .is_some()
        || user_repo::find_by_pending_email(db, email)
            .await
            .ok()
            .flatten()
            .is_some()
    {
        return AsterError::validation_error("email already exists");
    }

    AsterError::validation_error("username or email already exists")
}

fn map_user_email_db_err(err: DbErr) -> AsterError {
    if is_unique_conflict_db_err(&err) {
        AsterError::validation_error("email already exists")
    } else {
        AsterError::from(err)
    }
}

async fn map_contact_token_create_db_err<C: ConnectionTrait>(
    db: &C,
    err: DbErr,
    user_id: i64,
    purpose: VerificationPurpose,
) -> AsterError {
    if !is_unique_conflict_db_err(&err) {
        return AsterError::from(err);
    }

    if contact_verification_token_repo::find_latest_active_for_user(
        db,
        user_id,
        VerificationChannel::Email,
        purpose,
    )
    .await
    .ok()
    .flatten()
    .is_some()
    {
        return AsterError::rate_limited(ACTIVE_VERIFICATION_REQUEST_MESSAGE);
    }

    AsterError::from(err)
}

fn is_active_verification_request_error(err: &AsterError) -> bool {
    matches!(err, AsterError::RateLimited(message) if message == ACTIVE_VERIFICATION_REQUEST_MESSAGE)
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

    let username_for_err = username.clone();
    let email_for_err = email.clone();
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
    let user = match model.insert(db).await {
        Ok(user) => user,
        Err(err) => {
            return Err(map_user_create_db_err(db, err, &username_for_err, &email_for_err).await);
        }
    };

    if let Some(policy_group_id) = user.policy_group_id {
        state
            .policy_snapshot
            .set_user_policy_group(user.id, policy_group_id);
    }

    Ok(user)
}

async fn create_first_admin(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<user::Model> {
    tracing::info!("first user registered — granting admin role to '{username}'");
    create_user_with_role(
        &state.db,
        state,
        username,
        email,
        password,
        UserRole::Admin,
        UserStatus::Active,
        Some(Utc::now()),
    )
    .await
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

    match (contact_verification_token::ActiveModel {
        user_id: Set(user_id),
        channel: Set(VerificationChannel::Email),
        purpose: Set(purpose),
        target: Set(target.to_string()),
        token_hash: Set(token_hash),
        expires_at: Set(now + Duration::seconds(ttl_secs as i64)),
        consumed_at: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .insert(db)
    .await
    {
        Ok(_) => {}
        Err(err) => {
            return Err(map_contact_token_create_db_err(db, err, user_id, purpose).await);
        }
    }

    Ok(token)
}

async fn ensure_resend_allowed<C: ConnectionTrait>(
    state: &AppState,
    db: &C,
    user_id: i64,
    purpose: VerificationPurpose,
) -> Result<()> {
    if resend_allowed(state, db, user_id, purpose).await? {
        return Ok(());
    }

    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let remaining = policy.resend_cooldown_secs.max(1);
    Err(AsterError::rate_limited(format!(
        "please wait {remaining} seconds before resending",
    )))
}

async fn resend_allowed<C: ConnectionTrait>(
    state: &AppState,
    db: &C,
    user_id: i64,
    purpose: VerificationPurpose,
) -> Result<bool> {
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let Some(latest) = contact_verification_token_repo::find_latest_active_for_user(
        db,
        user_id,
        VerificationChannel::Email,
        purpose,
    )
    .await?
    else {
        return Ok(true);
    };

    let allowed_at = latest.created_at + Duration::seconds(policy.resend_cooldown_secs as i64);
    Ok(allowed_at <= Utc::now())
}

async fn password_reset_request_allowed<C: ConnectionTrait>(
    state: &AppState,
    db: &C,
    user_id: i64,
) -> Result<bool> {
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let Some(latest) = contact_verification_token_repo::find_latest_active_for_user(
        db,
        user_id,
        VerificationChannel::Email,
        VerificationPurpose::PasswordReset,
    )
    .await?
    else {
        return Ok(true);
    };

    let allowed_at =
        latest.created_at + Duration::seconds(policy.password_reset_request_cooldown_secs as i64);
    Ok(allowed_at <= Utc::now())
}

async fn update_password_in_connection<C: ConnectionTrait>(
    db: &C,
    user: user::Model,
    new_password: &str,
) -> Result<user::Model> {
    validate_password(new_password)?;

    let next_session_version = user.session_version.saturating_add(1);
    let mut active = user.into_active_model();
    active.password_hash = Set(hash::hash_password(new_password)?);
    active.session_version = Set(next_session_version);
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

pub async fn create_user_by_admin(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<AuthUserInfo> {
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
    .map(AuthUserInfo::from)
}

/// 注册用户，返回用户信息（不含密码）
pub async fn register(
    state: &AppState,
    username: &str,
    email: &str,
    password: &str,
) -> Result<AuthUserInfo> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    tracing::debug!(
        registration_enabled = auth_policy.allow_user_registration,
        activation_enabled = auth_policy.register_activation_enabled,
        "registering user"
    );
    if !auth_policy.allow_user_registration {
        return Err(AsterError::auth_forbidden(
            "new user registration is disabled",
        ));
    }

    if user_repo::count_all(&state.db).await? == 0 {
        return create_first_admin(state, username, email, password)
            .await
            .map(AuthUserInfo::from);
    }

    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let email_verified_at = (!auth_policy.register_activation_enabled).then_some(Utc::now());
    let user = create_user_with_role(
        &txn,
        state,
        username,
        email,
        password,
        UserRole::User,
        UserStatus::Active,
        email_verified_at,
    )
    .await?;
    if auth_policy.register_activation_enabled {
        let token = issue_contact_verification_token(
            &txn,
            user.id,
            VerificationPurpose::RegisterActivation,
            &user.email,
            policy.register_activation_ttl_secs,
        )
        .await?;
        mail_outbox_service::enqueue(
            &txn,
            &user.email,
            Some(&user.username),
            MailTemplatePayload::register_activation(&user.username, &token),
        )
        .await?;
    }
    txn.commit().await.map_err(AsterError::from)?;

    tracing::debug!(
        user_id = user.id,
        activation_enabled = auth_policy.register_activation_enabled,
        email_verified = user.email_verified_at.is_some(),
        "registered user"
    );
    Ok(AuthUserInfo::from(user))
}

pub async fn resend_register_activation(
    state: &AppState,
    identifier: &str,
) -> Result<Option<UserAuditInfo>> {
    let Some(user) = find_user_by_identifier(&state.db, identifier).await? else {
        return Ok(None);
    };

    if !user.status.is_active() || is_email_verified(&user) {
        return Ok(None);
    }

    if !resend_allowed(
        state,
        &state.db,
        user.id,
        VerificationPurpose::RegisterActivation,
    )
    .await?
    {
        tracing::debug!(
            user_id = user.id,
            "register activation resend skipped due to cooldown"
        );
        return Ok(None);
    }
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let token = match issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::RegisterActivation,
        &user.email,
        policy.register_activation_ttl_secs,
    )
    .await
    {
        Ok(token) => token,
        Err(err) if is_active_verification_request_error(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    mail_outbox_service::enqueue(
        &txn,
        &user.email,
        Some(&user.username),
        MailTemplatePayload::register_activation(&user.username, &token),
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(Some(user_audit_info(&user)))
}

pub async fn request_email_change(
    state: &AppState,
    user_id: i64,
    new_email: &str,
) -> Result<AuthUserInfo> {
    tracing::debug!(user_id, "requesting email change");
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
    let updated = active.update(&txn).await.map_err(map_user_email_db_err)?;
    let token = issue_contact_verification_token(
        &txn,
        updated.id,
        VerificationPurpose::ContactChange,
        &normalized_email,
        policy.contact_change_ttl_secs,
    )
    .await?;
    mail_outbox_service::enqueue(
        &txn,
        &normalized_email,
        Some(&updated.username),
        MailTemplatePayload::contact_change_confirmation(&updated.username, &token),
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    tracing::debug!(
        user_id = updated.id,
        has_pending_email = updated.pending_email.is_some(),
        "requested email change"
    );
    Ok(AuthUserInfo::from(updated))
}

pub async fn resend_email_change(state: &AppState, user_id: i64) -> Result<Option<UserAuditInfo>> {
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
    if !resend_allowed(
        state,
        &state.db,
        user.id,
        VerificationPurpose::ContactChange,
    )
    .await?
    {
        tracing::debug!(
            user_id = user.id,
            "email change resend skipped due to cooldown"
        );
        return Ok(None);
    }
    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let token = match issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::ContactChange,
        &pending_email,
        policy.contact_change_ttl_secs,
    )
    .await
    {
        Ok(token) => token,
        Err(err) if is_active_verification_request_error(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    mail_outbox_service::enqueue(
        &txn,
        &pending_email,
        Some(&user.username),
        MailTemplatePayload::contact_change_confirmation(&user.username, &token),
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    Ok(Some(user_audit_info(&user)))
}

pub async fn request_password_reset(
    state: &AppState,
    email: &str,
) -> Result<PasswordResetRequestResult> {
    tracing::debug!("requesting password reset");
    let normalized_email = normalize_email(email)?;
    let Some(user) = user_repo::find_by_email(&state.db, &normalized_email).await? else {
        return Ok(PasswordResetRequestResult { user: None });
    };

    if !user.status.is_active() || !is_email_verified(&user) {
        return Ok(PasswordResetRequestResult { user: None });
    }

    if !password_reset_request_allowed(state, &state.db, user.id).await? {
        tracing::debug!(
            user_id = user.id,
            "password reset request skipped due to cooldown"
        );
        return Ok(PasswordResetRequestResult {
            user: Some(user_audit_info(&user)),
        });
    }

    let policy = RuntimeContactVerificationPolicy::from_runtime_config(&state.runtime_config);
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let token = match issue_contact_verification_token(
        &txn,
        user.id,
        VerificationPurpose::PasswordReset,
        &user.email,
        policy.password_reset_ttl_secs,
    )
    .await
    {
        Ok(token) => token,
        Err(err) if is_active_verification_request_error(&err) => {
            return Ok(PasswordResetRequestResult {
                user: Some(user_audit_info(&user)),
            });
        }
        Err(err) => return Err(err),
    };
    mail_outbox_service::enqueue(
        &txn,
        &user.email,
        Some(&user.username),
        MailTemplatePayload::password_reset(&user.username, &token),
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;

    tracing::debug!(user_id = user.id, "enqueued password reset");
    Ok(PasswordResetRequestResult {
        user: Some(user_audit_info(&user)),
    })
}

pub async fn confirm_password_reset(
    state: &AppState,
    token: &str,
    new_password: &str,
) -> Result<AuthUserInfo> {
    tracing::debug!("confirming password reset");
    validate_password(new_password)?;

    let token_hash = hash::sha256_hex(token.as_bytes());
    let record = contact_verification_token_repo::find_by_token_hash(&state.db, &token_hash)
        .await?
        .ok_or_else(|| {
            AsterError::contact_verification_invalid("password reset link is invalid")
        })?;

    if record.purpose != VerificationPurpose::PasswordReset {
        return Err(AsterError::contact_verification_invalid(
            "password reset link is invalid",
        ));
    }
    if record.consumed_at.is_some() {
        return Err(AsterError::contact_verification_invalid(
            "password reset link has already been used",
        ));
    }
    if record.expires_at <= Utc::now() {
        return Err(AsterError::contact_verification_expired(
            "password reset link has expired",
        ));
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing_user = user_repo::find_by_id(&txn, record.user_id).await?;
    if !existing_user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if !is_email_verified(&existing_user) || existing_user.email != record.target {
        return Err(AsterError::contact_verification_invalid(
            "password reset request no longer exists",
        ));
    }

    let consumed =
        contact_verification_token_repo::mark_consumed_if_unused(&txn, record.id).await?;
    if !consumed {
        return Err(AsterError::contact_verification_invalid(
            "password reset link has already been used",
        ));
    }

    let updated = update_password_in_connection(&txn, existing_user, new_password).await?;
    mail_outbox_service::enqueue(
        &txn,
        &updated.email,
        Some(&updated.username),
        MailTemplatePayload::password_reset_notice(&updated.username),
    )
    .await?;
    txn.commit().await.map_err(AsterError::from)?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    tracing::debug!(
        user_id = updated.id,
        session_version = updated.session_version,
        "confirmed password reset"
    );
    Ok(AuthUserInfo::from(updated))
}

pub async fn confirm_contact_verification(
    state: &AppState,
    token: &str,
) -> Result<ContactVerificationConfirmResult> {
    tracing::debug!("confirming contact verification");
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
    tracing::debug!(user_id, purpose = ?purpose, "loaded contact verification record");

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing_user = user_repo::find_by_id(&txn, user_id).await?;
    if !existing_user.status.is_active() {
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    let username = existing_user.username.clone();
    let previous_email = (purpose == VerificationPurpose::ContactChange
        && existing_user.email != target)
        .then(|| existing_user.email.clone());
    if purpose == VerificationPurpose::PasswordReset {
        return Err(AsterError::contact_verification_invalid(
            "password reset token cannot be confirmed from this endpoint",
        ));
    }

    let consumed =
        contact_verification_token_repo::mark_consumed_if_unused(&txn, record.id).await?;
    if !consumed {
        return Err(AsterError::contact_verification_invalid(
            "contact verification link has already been used",
        ));
    }

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
                active.update(&txn).await.map_err(map_user_email_db_err)?;
                if let Some(previous_email) = previous_email.as_deref() {
                    mail_outbox_service::enqueue(
                        &txn,
                        previous_email,
                        Some(&username),
                        MailTemplatePayload::contact_change_notice(
                            &username,
                            previous_email,
                            &target,
                        ),
                    )
                    .await?;
                }
            }
        }
        VerificationPurpose::PasswordReset => unreachable!("handled above"),
    }
    txn.commit().await.map_err(AsterError::from)?;

    tracing::debug!(user_id, purpose = ?purpose, "confirmed contact verification");
    Ok(ContactVerificationConfirmResult {
        purpose,
        user_id,
        target,
    })
}

/// 检查公开认证状态（仅返回实例级状态，不暴露标识符是否存在）
pub async fn check_auth_state(state: &AppState) -> Result<bool> {
    Ok(user_repo::count_all(&state.db).await? > 0)
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
) -> Result<AuthUserInfo> {
    let db = &state.db;
    tracing::debug!("running initial setup");
    if user_repo::count_all(db).await? > 0 {
        return Err(AsterError::validation_error("system already initialized"));
    }
    let user = create_first_admin(state, username, email, password)
        .await
        .map(AuthUserInfo::from)?;
    tracing::debug!(user_id = user.id, "completed initial setup");
    Ok(user)
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

/// 登录，返回 tokens + user_id
/// identifier 支持邮箱或用户名
pub async fn login(state: &AppState, identifier: &str, password: &str) -> Result<LoginResult> {
    let db = &state.db;
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let identifier_kind = if identifier.trim().contains('@') {
        "email"
    } else {
        "username"
    };
    tracing::debug!(identifier_kind, "login attempt");

    let Some(user) = find_user_by_identifier(db, identifier).await? else {
        tracing::debug!(identifier_kind, "login rejected: user not found");
        return Err(AsterError::auth_invalid_credentials("user not found"));
    };

    if !user.status.is_active() {
        tracing::debug!(user_id = user.id, "login rejected: account disabled");
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if !is_email_verified(&user) {
        tracing::debug!(
            user_id = user.id,
            "login rejected: account pending activation"
        );
        return Err(AsterError::auth_pending_activation(
            "account pending activation",
        ));
    }

    if !hash::verify_password(password, &user.password_hash)? {
        tracing::debug!(user_id = user.id, "login rejected: invalid password");
        return Err(AsterError::auth_invalid_credentials("wrong password"));
    }

    let (access, refresh) = issue_tokens(
        user.id,
        user.session_version,
        &state.config.auth.jwt_secret,
        auth_policy,
    )?;

    tracing::debug!(
        user_id = user.id,
        session_version = user.session_version,
        "login succeeded"
    );

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
) -> Result<AuthUserInfo> {
    tracing::debug!(user_id, "changing password");
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
) -> Result<AuthUserInfo> {
    tracing::debug!(user_id, "setting password");
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let updated = update_password_in_connection(&state.db, user, new_password).await?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    tracing::debug!(
        user_id = updated.id,
        session_version = updated.session_version,
        "set password"
    );
    Ok(AuthUserInfo::from(updated))
}

pub async fn cleanup_expired_contact_verification_tokens(state: &AppState) -> Result<u64> {
    contact_verification_token_repo::delete_expired(&state.db).await
}

/// 用 refresh token 换 access token
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
