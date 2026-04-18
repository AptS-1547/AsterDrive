//! 认证服务聚合入口。

mod contact_verification;
mod password;
mod registration;
mod session;
mod shared;
mod tokens;
mod validation;

use sea_orm::{ActiveValue, Set};
use serde::{Deserialize, Serialize};

use crate::entities::user;
use crate::types::{StoredUserConfig, TokenType, UserRole, UserStatus, VerificationPurpose};

pub use contact_verification::{
    cleanup_expired_contact_verification_tokens, confirm_contact_verification,
    confirm_password_reset, request_email_change, request_password_reset, resend_email_change,
};
pub use password::{change_password, login, set_password};
pub use registration::{
    check_auth_state, create_user_by_admin, register, resend_register_activation, setup,
};
pub use session::{get_auth_snapshot, invalidate_auth_snapshot_cache, revoke_user_sessions};
pub use tokens::{
    authenticate_access_token, authenticate_refresh_token, issue_tokens_for_session,
    issue_tokens_for_user, refresh_token, verify_token,
};
pub(crate) use validation::{validate_email, validate_password, validate_username};

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
    pub config: Option<StoredUserConfig>,
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

#[derive(Debug)]
pub struct LoginResult {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: i64,
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

pub fn is_email_verified(user: &user::Model) -> bool {
    user.email_verified_at.is_some()
}
