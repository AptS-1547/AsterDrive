//! 认证 API 路由：`public`。

use super::{
    ActionMessageResp, CheckResp, ContactVerificationConfirmQuery,
    ContactVerificationRedirectStatus, PasswordResetConfirmReq, PasswordResetRequestReq,
    RegisterReq, ResendRegisterActivationReq, SetupReq, apply_auth_mail_response_floor,
    contact_verification_redirect_response, request_has_active_access_session,
};
use crate::api::response::ApiResponse;
use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service, user_service};
use crate::types::VerificationPurpose;
use actix_web::{HttpRequest, HttpResponse, web};

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/check",
    tag = "auth",
    operation_id = "check_auth_state",
    responses(
        (status = 200, description = "Check result", body = inline(ApiResponse<CheckResp>)),
    ),
)]
pub async fn check(state: web::Data<AppState>) -> Result<HttpResponse> {
    let has_users = auth_service::check_auth_state(&state).await?;
    let allow_user_registration =
        RuntimeAuthPolicy::from_runtime_config(&state.runtime_config).allow_user_registration;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(CheckResp {
        has_users,
        allow_user_registration,
    })))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/setup",
    tag = "auth",
    operation_id = "setup",
    request_body = SetupReq,
    responses(
        (status = 201, description = "Admin account created", body = inline(ApiResponse<crate::api::routes::auth::UserInfo>)),
        (status = 400, description = "System already initialized"),
    ),
)]
pub async fn setup(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<SetupReq>,
) -> Result<HttpResponse> {
    let user = auth_service::setup(&state, &body.username, &body.email, &body.password).await?;
    let user_info = user_service::get_self_info(&state, user.id).await?;
    let ctx = audit_service::AuditContext {
        user_id: user.id,
        ip_address: req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string()),
        user_agent: req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    };
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::SystemSetup,
        None,
        None,
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user_info)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    operation_id = "register",
    request_body = RegisterReq,
    responses(
        (status = 201, description = "Registration successful", body = inline(ApiResponse<crate::api::routes::auth::UserInfo>)),
        (status = 400, description = "Validation error"),
    ),
)]
pub async fn register(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RegisterReq>,
) -> Result<HttpResponse> {
    let user = auth_service::register(&state, &body.username, &body.email, &body.password).await?;
    let user_info = user_service::get_self_info(&state, user.id).await?;
    let ctx = audit_service::AuditContext {
        user_id: user.id,
        ip_address: req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string()),
        user_agent: req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    };
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::UserRegister,
        None,
        None,
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user_info)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/register/resend",
    tag = "auth",
    operation_id = "resend_register_activation",
    request_body = ResendRegisterActivationReq,
    responses(
        (status = 200, description = "Activation resend request accepted", body = inline(ApiResponse<ActionMessageResp>)),
    ),
)]
pub async fn resend_register_activation(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ResendRegisterActivationReq>,
) -> Result<HttpResponse> {
    let started_at = tokio::time::Instant::now();
    let result = auth_service::resend_register_activation(&state, &body.identifier).await;
    let user = match result {
        Ok(user) => user,
        Err(error) => {
            apply_auth_mail_response_floor(started_at).await;
            return Err(error);
        }
    };
    if let Some(user) = user {
        let ctx = audit_service::AuditContext {
            user_id: user.id,
            ip_address: req
                .connection_info()
                .realip_remote_addr()
                .map(|s| s.to_string()),
            user_agent: req
                .headers()
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
        };
        audit_service::log(
            &state,
            &ctx,
            audit_service::AuditAction::UserResendRegistration,
            Some("user"),
            Some(user.id),
            Some(&user.username),
            None,
        )
        .await;
    }
    apply_auth_mail_response_floor(started_at).await;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(ActionMessageResp {
        message: "If the account can be reactivated, an activation email will be sent".to_string(),
    })))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/auth/contact-verification/confirm",
    tag = "auth",
    operation_id = "confirm_contact_verification",
    params(ContactVerificationConfirmQuery),
    responses(
        (status = 302, description = "Verification consumed and browser redirected to the frontend"),
    ),
)]
pub async fn confirm_contact_verification(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ContactVerificationConfirmQuery>,
) -> Result<HttpResponse> {
    let has_active_session = request_has_active_access_session(&state, &req).await;
    let fallback_path = if has_active_session {
        "/settings/security"
    } else {
        "/login"
    };
    let Some(token) = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    else {
        return Ok(contact_verification_redirect_response(
            &state,
            fallback_path,
            ContactVerificationRedirectStatus::Missing,
            None,
        ));
    };

    let result = match auth_service::confirm_contact_verification(&state, token).await {
        Ok(result) => result,
        Err(AsterError::ContactVerificationInvalid(_)) => {
            return Ok(contact_verification_redirect_response(
                &state,
                fallback_path,
                ContactVerificationRedirectStatus::Invalid,
                None,
            ));
        }
        Err(AsterError::ContactVerificationExpired(_)) => {
            return Ok(contact_verification_redirect_response(
                &state,
                fallback_path,
                ContactVerificationRedirectStatus::Expired,
                None,
            ));
        }
        Err(error) => return Err(error),
    };

    if result.purpose == VerificationPurpose::PasswordReset {
        return Ok(contact_verification_redirect_response(
            &state,
            fallback_path,
            ContactVerificationRedirectStatus::Invalid,
            None,
        ));
    }

    let action = match result.purpose {
        VerificationPurpose::RegisterActivation => {
            audit_service::AuditAction::UserConfirmRegistration
        }
        VerificationPurpose::ContactChange => audit_service::AuditAction::UserConfirmEmailChange,
        VerificationPurpose::PasswordReset => audit_service::AuditAction::UserConfirmPasswordReset,
    };
    let ctx = audit_service::AuditContext {
        user_id: result.user_id,
        ip_address: req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string()),
        user_agent: req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    };
    audit_service::log(
        &state,
        &ctx,
        action,
        Some("user"),
        Some(result.user_id),
        None,
        None,
    )
    .await;

    let (redirect_path, redirect_status, email) = match result.purpose {
        VerificationPurpose::RegisterActivation if has_active_session => (
            "/settings/security",
            ContactVerificationRedirectStatus::RegisterActivated,
            None,
        ),
        VerificationPurpose::RegisterActivation => (
            "/login",
            ContactVerificationRedirectStatus::RegisterActivated,
            None,
        ),
        VerificationPurpose::ContactChange if has_active_session => (
            "/settings/security",
            ContactVerificationRedirectStatus::EmailChanged,
            Some(result.target.as_str()),
        ),
        VerificationPurpose::ContactChange => (
            "/login",
            ContactVerificationRedirectStatus::EmailChanged,
            Some(result.target.as_str()),
        ),
        VerificationPurpose::PasswordReset => (
            fallback_path,
            ContactVerificationRedirectStatus::Invalid,
            None,
        ),
    };

    Ok(contact_verification_redirect_response(
        &state,
        redirect_path,
        redirect_status,
        email,
    ))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/password/reset/request",
    tag = "auth",
    operation_id = "request_password_reset",
    request_body = PasswordResetRequestReq,
    responses(
        (status = 200, description = "Password reset request accepted", body = inline(ApiResponse<ActionMessageResp>)),
        (status = 400, description = "Invalid email input"),
    ),
)]
pub async fn request_password_reset(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<PasswordResetRequestReq>,
) -> Result<HttpResponse> {
    let started_at = tokio::time::Instant::now();
    let result = auth_service::request_password_reset(&state, &body.email).await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            apply_auth_mail_response_floor(started_at).await;
            return Err(error);
        }
    };
    if let Some(user) = result.user.as_ref() {
        let ctx = audit_service::AuditContext {
            user_id: user.id,
            ip_address: req
                .connection_info()
                .realip_remote_addr()
                .map(|s| s.to_string()),
            user_agent: req
                .headers()
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
        };
        audit_service::log(
            &state,
            &ctx,
            audit_service::AuditAction::UserRequestPasswordReset,
            Some("user"),
            Some(user.id),
            Some(&user.username),
            None,
        )
        .await;
    }
    apply_auth_mail_response_floor(started_at).await;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(ActionMessageResp {
        message: "If the account is eligible, a password reset email will be sent".to_string(),
    })))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/password/reset/confirm",
    tag = "auth",
    operation_id = "confirm_password_reset",
    request_body = PasswordResetConfirmReq,
    responses(
        (status = 200, description = "Password reset successful", body = inline(ApiResponse<ActionMessageResp>)),
        (status = 400, description = "Invalid token or password"),
        (status = 410, description = "Reset token expired"),
    ),
)]
pub async fn confirm_password_reset(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<PasswordResetConfirmReq>,
) -> Result<HttpResponse> {
    let user =
        auth_service::confirm_password_reset(&state, &body.token, &body.new_password).await?;

    let ctx = audit_service::AuditContext {
        user_id: user.id,
        ip_address: req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string()),
        user_agent: req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    };
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::UserConfirmPasswordReset,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        None,
    )
    .await;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(ActionMessageResp {
        message: "Password reset successful".to_string(),
    })))
}
