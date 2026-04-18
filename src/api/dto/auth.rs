use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

/// Registration request for new users.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// Resend registration activation email.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ResendRegisterActivationReq {
    pub identifier: String,
}

/// Response for the `/auth/check` endpoint.
#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CheckResp {
    pub has_users: bool,
    pub allow_user_registration: bool,
}

/// Initial system setup (first admin account).
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetupReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// Standard login credentials.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct LoginReq {
    pub identifier: String,
    pub password: String,
}

/// Query parameters for email contact verification confirmation.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct ContactVerificationConfirmQuery {
    pub token: Option<String>,
}

/// Response body for token issuance (login / refresh / password change).
#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AuthTokenResp {
    pub expires_in: u64,
}

/// Generic message-only response (used after email operations).
#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ActionMessageResp {
    pub message: String,
}

/// Update the user's avatar source.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdateAvatarSourceReq {
    pub source: crate::types::AvatarSource,
}

/// Update display name in user profile.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdateProfileReq {
    pub display_name: Option<String>,
}

/// Change the authenticated user's password.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

/// Request a password reset email.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PasswordResetRequestReq {
    pub email: String,
}

/// Confirm a password reset with the token from the email.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PasswordResetConfirmReq {
    pub token: String,
    pub new_password: String,
}

/// Request an email address change.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RequestEmailChangeReq {
    pub new_email: String,
}
