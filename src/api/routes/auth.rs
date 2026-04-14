use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::config::site_url;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    audit_service, auth_service, profile_service, storage_change_service, team_service,
};
use crate::types::VerificationPurpose;
use actix_governor::Governor;
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::http::header;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;
use rand::RngExt;
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

use crate::api::middleware::rate_limit;
use crate::config::auth_runtime::RuntimeAuthPolicy;

// Re-export preference types from user_service for OpenAPI schema registration.
pub use crate::services::user_service::{
    ColorPreset, Language, MeResponse, PrefViewMode, ThemeMode, UpdatePreferencesReq, UserInfo,
    UserPreferences,
};

use crate::services::auth_service::Claims;
pub use crate::services::profile_service::{AvatarInfo, UserProfileInfo};
use crate::services::user_service;
pub use crate::types::AvatarSource;

const ACCESS_COOKIE: &str = "aster_access";
const REFRESH_COOKIE: &str = "aster_refresh";
const ACCESS_COOKIE_PATH: &str = "/";
const REFRESH_COOKIE_PATH: &str = "/api/v1/auth/refresh";
const AUTH_MAIL_RESPONSE_FLOOR_MS: u64 = 350;
const AUTH_MAIL_RESPONSE_JITTER_MS: u64 = 125;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.auth);

    // 公开路由 + 认证路由分别注册到 /auth 路径下
    web::scope("/auth")
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/check", web::post().to(check))
        .route("/register", web::post().to(register))
        .route(
            "/register/resend",
            web::post().to(resend_register_activation),
        )
        .route("/setup", web::post().to(setup))
        .route(
            "/contact-verification/confirm",
            web::get().to(confirm_contact_verification),
        )
        .route(
            "/password/reset/request",
            web::post().to(request_password_reset),
        )
        .route(
            "/password/reset/confirm",
            web::post().to(confirm_password_reset),
        )
        .route("/login", web::post().to(login))
        .route("/refresh", web::post().to(refresh))
        .route("/logout", web::post().to(logout))
        // 需要认证的端点使用嵌套 scope，注意路径前缀不能重复
        .service(
            web::scope("")
                .wrap(crate::api::middleware::auth::JwtAuth)
                .route("/me", web::get().to(me))
                .route("/password", web::put().to(put_password))
                .route("/email/change", web::post().to(request_email_change))
                .route("/email/change/resend", web::post().to(resend_email_change))
                .route("/preferences", web::patch().to(patch_preferences))
                .route("/profile", web::patch().to(patch_profile))
                .route("/profile/avatar/upload", web::post().to(upload_avatar))
                .route("/profile/avatar/source", web::put().to(put_avatar_source))
                .route("/events/storage", web::get().to(get_storage_events))
                .route("/profile/avatar/{size}", web::get().to(get_self_avatar)),
        )
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ResendRegisterActivationReq {
    pub identifier: String,
}

#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CheckResp {
    pub has_users: bool,
    pub allow_user_registration: bool,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetupReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct LoginReq {
    pub identifier: String,
    pub password: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct ContactVerificationConfirmQuery {
    pub token: Option<String>,
}

#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AuthTokenResp {
    pub expires_in: u64,
}

#[derive(serde::Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ActionMessageResp {
    pub message: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdateAvatarSourceReq {
    pub source: AvatarSource,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdateProfileReq {
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PasswordResetRequestReq {
    pub email: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PasswordResetConfirmReq {
    pub token: String,
    pub new_password: String,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RequestEmailChangeReq {
    pub new_email: String,
}

/// 构建 HttpOnly cookie
fn build_cookie(
    name: &str,
    path: &str,
    value: &str,
    max_age_secs: i64,
    secure: bool,
) -> Cookie<'static> {
    Cookie::build(name.to_string(), value.to_string())
        .path(path.to_string())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(CookieDuration::seconds(max_age_secs))
        .finish()
}

/// 构建清除 cookie
fn clear_cookie(name: &str, path: &str, secure: bool) -> Cookie<'static> {
    Cookie::build(name.to_string(), "")
        .path(path.to_string())
        .http_only(true)
        .secure(secure)
        .max_age(CookieDuration::ZERO)
        .finish()
}

fn build_access_cookie(value: &str, max_age_secs: i64, secure: bool) -> Cookie<'static> {
    build_cookie(
        ACCESS_COOKIE,
        ACCESS_COOKIE_PATH,
        value,
        max_age_secs,
        secure,
    )
}

fn build_refresh_cookie(value: &str, max_age_secs: i64, secure: bool) -> Cookie<'static> {
    build_cookie(
        REFRESH_COOKIE,
        REFRESH_COOKIE_PATH,
        value,
        max_age_secs,
        secure,
    )
}

fn clear_access_cookie(secure: bool) -> Cookie<'static> {
    clear_cookie(ACCESS_COOKIE, ACCESS_COOKIE_PATH, secure)
}

fn clear_refresh_cookie(secure: bool) -> Cookie<'static> {
    clear_cookie(REFRESH_COOKIE, REFRESH_COOKIE_PATH, secure)
}

async fn apply_auth_mail_response_floor(started_at: tokio::time::Instant) {
    let mut rng = rand::rng();
    let jitter_ms = rng.random_range(0..=AUTH_MAIL_RESPONSE_JITTER_MS);
    let target = std::time::Duration::from_millis(AUTH_MAIL_RESPONSE_FLOOR_MS + jitter_ms);
    let elapsed = started_at.elapsed();
    if elapsed < target {
        tokio::time::sleep(target - elapsed).await;
    }
}

fn bearer_token(req: &actix_web::HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
}

#[derive(Clone, Copy)]
enum ContactVerificationRedirectStatus {
    EmailChanged,
    Expired,
    Invalid,
    Missing,
    RegisterActivated,
}

impl ContactVerificationRedirectStatus {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::EmailChanged => "email-changed",
            Self::Expired => "expired",
            Self::Invalid => "invalid",
            Self::Missing => "missing",
            Self::RegisterActivated => "register-activated",
        }
    }
}

async fn request_has_active_access_session(state: &AppState, req: &HttpRequest) -> bool {
    let token = req
        .cookie(ACCESS_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .or_else(|| bearer_token(req));

    let Some(token) = token else {
        return false;
    };

    auth_service::authenticate_access_token(state, &token)
        .await
        .is_ok()
}

fn contact_verification_redirect_url(
    state: &AppState,
    path: &str,
    status: ContactVerificationRedirectStatus,
    email: Option<&str>,
) -> String {
    let mut redirect_path = format!("{path}?contact_verification={}", status.as_query_value());

    if let Some(email) = email {
        redirect_path.push_str("&email=");
        redirect_path.push_str(&urlencoding::encode(email));
    }

    site_url::public_app_url_or_path(&state.runtime_config, &redirect_path)
}

fn contact_verification_redirect_response(
    state: &AppState,
    path: &str,
    status: ContactVerificationRedirectStatus,
    email: Option<&str>,
) -> HttpResponse {
    HttpResponse::Found()
        .append_header((
            header::LOCATION,
            contact_verification_redirect_url(state, path, status, email),
        ))
        .finish()
}

fn storage_event_frame(event: &storage_change_service::StorageChangeEvent) -> Option<Bytes> {
    serde_json::to_string(event)
        .map(|json| Bytes::from(format!("data: {json}\n\n")))
        .map_err(|e| tracing::warn!("failed to serialize storage change event: {e}"))
        .ok()
}

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
        (status = 201, description = "Admin account created", body = inline(ApiResponse<UserInfo>)),
        (status = 400, description = "System already initialized"),
    ),
)]
pub async fn setup(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
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

pub async fn get_storage_events(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = claims.user_id;
    let visible_team_ids = team_service::list_user_team_ids(&state, user_id, false).await?;
    let mut rx = state.storage_change_tx.subscribe();

    let stream = async_stream::stream! {
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(15));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    yield Ok::<Bytes, actix_web::Error>(Bytes::from_static(b": keep-alive\n\n"));
                }
                recv = rx.recv() => {
                    match recv {
                        Ok(event) => {
                            if !event.is_visible_to(user_id, &visible_team_ids) {
                                continue;
                            }
                            if let Some(frame) = storage_event_frame(&event) {
                                yield Ok(frame);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(user_id, skipped, "storage change event stream lagged");
                            if let Some(frame) = storage_event_frame(
                                &storage_change_service::StorageChangeEvent::sync_required(),
                            ) {
                                yield Ok(frame);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Content-Encoding", "identity"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    operation_id = "register",
    request_body = RegisterReq,
    responses(
        (status = 201, description = "Registration successful", body = inline(ApiResponse<UserInfo>)),
        (status = 400, description = "Validation error"),
    ),
)]
pub async fn register(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
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
    req: actix_web::HttpRequest,
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

    let action = match result.purpose {
        VerificationPurpose::RegisterActivation => {
            audit_service::AuditAction::UserConfirmRegistration
        }
        VerificationPurpose::ContactChange => audit_service::AuditAction::UserConfirmEmailChange,
        VerificationPurpose::PasswordReset => unreachable!("handled in password reset flow"),
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
        VerificationPurpose::PasswordReset => unreachable!("handled in password reset flow"),
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
    req: actix_web::HttpRequest,
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
    req: actix_web::HttpRequest,
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

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    operation_id = "login",
    request_body = LoginReq,
    responses(
        (status = 200, description = "Login successful, tokens set in HttpOnly cookies", body = inline(ApiResponse<AuthTokenResp>)),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    body: web::Json<LoginReq>,
) -> Result<HttpResponse> {
    let result = auth_service::login(&state, &body.identifier, &body.password).await?;
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);

    // 审计日志 — 直接使用 login 返回的 user_id
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
        audit_service::AuditAction::UserLogin,
        None,
        None,
        Some(&body.identifier),
        None,
    )
    .await;

    let secure = auth_policy.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(
            &result.access_token,
            auth_policy.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_refresh_cookie(
            &result.refresh_token,
            auth_policy.refresh_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::ok(AuthTokenResp {
            expires_in: auth_policy.access_token_ttl_secs,
        })))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    operation_id = "refresh",
    responses(
        (status = 200, description = "Token refreshed, new access token set in HttpOnly cookie", body = inline(ApiResponse<AuthTokenResp>)),
        (status = 401, description = "Invalid refresh token"),
    ),
)]
pub async fn refresh(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let refresh_tok = req
        .cookie(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| crate::errors::AsterError::auth_token_invalid("missing refresh cookie"))?;

    let access = auth_service::refresh_token(&state, &refresh_tok).await?;

    let secure = auth_policy.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(
            &access,
            auth_policy.access_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::ok(AuthTokenResp {
            expires_in: auth_policy.access_token_ttl_secs,
        })))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    operation_id = "logout",
    responses(
        (status = 200, description = "Logged out, cookies cleared"),
    ),
)]
pub async fn logout(state: web::Data<AppState>, req: actix_web::HttpRequest) -> HttpResponse {
    for token in [
        req.cookie(REFRESH_COOKIE)
            .map(|cookie| cookie.value().to_string()),
        req.cookie(ACCESS_COOKIE)
            .map(|cookie| cookie.value().to_string()),
        bearer_token(&req),
    ]
    .into_iter()
    .flatten()
    {
        let Ok(claims) = auth_service::verify_token(&token, &state.config.auth.jwt_secret) else {
            continue;
        };

        let ctx = audit_service::AuditContext {
            user_id: claims.user_id,
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
            audit_service::AuditAction::UserLogout,
            None,
            None,
            None,
            None,
        )
        .await;
        break;
    }

    let secure = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config).cookie_secure;
    HttpResponse::Ok()
        .cookie(clear_access_cookie(secure))
        .cookie(clear_refresh_cookie(secure))
        .json(ApiResponse::<()>::ok_empty())
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    operation_id = "me",
    responses(
        (status = 200, description = "Current user info", body = inline(ApiResponse<MeResponse>)),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn me(state: web::Data<AppState>, claims: web::ReqData<Claims>) -> Result<HttpResponse> {
    let resp = user_service::get_me(&state, claims.user_id, claims.exp as i64).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/auth/password",
    tag = "auth",
    operation_id = "change_password",
    request_body = ChangePasswordReq,
    responses(
        (status = 200, description = "Password updated", body = inline(ApiResponse<AuthTokenResp>)),
        (status = 400, description = "Invalid new password"),
        (status = 401, description = "Current password is invalid"),
    ),
    security(("bearer" = [])),
)]
pub async fn put_password(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    claims: web::ReqData<Claims>,
    body: web::Json<ChangePasswordReq>,
) -> Result<HttpResponse> {
    let user = auth_service::change_password(
        &state,
        claims.user_id,
        &body.current_password,
        &body.new_password,
    )
    .await?;
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let (access_token, refresh_token) =
        auth_service::issue_tokens_for_session(&state, user.id, user.session_version)?;

    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::UserChangePassword,
        None,
        None,
        None,
        None,
    )
    .await;

    let secure = auth_policy.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(
            &access_token,
            auth_policy.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_refresh_cookie(
            &refresh_token,
            auth_policy.refresh_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::ok(AuthTokenResp {
            expires_in: auth_policy.access_token_ttl_secs,
        })))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/email/change",
    tag = "auth",
    operation_id = "request_email_change",
    request_body = RequestEmailChangeReq,
    responses(
        (status = 200, description = "Email change requested", body = inline(ApiResponse<UserInfo>)),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Account pending activation"),
    ),
    security(("bearer" = [])),
)]
pub async fn request_email_change(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    claims: web::ReqData<Claims>,
    body: web::Json<RequestEmailChangeReq>,
) -> Result<HttpResponse> {
    let user = auth_service::request_email_change(&state, claims.user_id, &body.new_email).await?;
    let user_info = user_service::get_self_info(&state, user.id).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::UserRequestEmailChange,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user_info)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/email/change/resend",
    tag = "auth",
    operation_id = "resend_email_change",
    responses(
        (status = 200, description = "Email change confirmation resend request accepted", body = inline(ApiResponse<ActionMessageResp>)),
        (status = 400, description = "No pending email change"),
    ),
    security(("bearer" = [])),
)]
pub async fn resend_email_change(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let started_at = tokio::time::Instant::now();
    let result = auth_service::resend_email_change(&state, claims.user_id).await;
    let user = match result {
        Ok(user) => user,
        Err(error) => {
            apply_auth_mail_response_floor(started_at).await;
            return Err(error);
        }
    };
    if let Some(user) = user {
        let ctx = audit_service::AuditContext::from_request(&req, &claims);
        audit_service::log(
            &state,
            &ctx,
            audit_service::AuditAction::UserResendEmailChange,
            Some("user"),
            Some(user.id),
            Some(&user.username),
            None,
        )
        .await;
    }
    apply_auth_mail_response_floor(started_at).await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(ActionMessageResp {
        message: "If an email change is pending, a confirmation email will be sent".to_string(),
    })))
}

/// Update the current user's preferences.
///
/// Only non-null fields in the request body are merged into the existing
/// preferences. Returns the full updated preferences object.
#[api_docs_macros::path(
    patch,
    path = "/api/v1/auth/preferences",
    tag = "auth",
    operation_id = "update_preferences",
    request_body = UpdatePreferencesReq,
    responses(
        (status = 200, description = "Preferences updated", body = inline(ApiResponse<UserPreferences>)),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_preferences(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdatePreferencesReq>,
) -> Result<HttpResponse> {
    let prefs = user_service::update_preferences(&state, claims.user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(prefs)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/auth/profile",
    tag = "auth",
    operation_id = "update_profile",
    request_body = UpdateProfileReq,
    responses(
        (status = 200, description = "Profile updated", body = inline(ApiResponse<UserProfileInfo>)),
        (status = 400, description = "Invalid profile input"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_profile(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdateProfileReq>,
) -> Result<HttpResponse> {
    let profile =
        profile_service::update_profile(&state, claims.user_id, body.display_name.clone()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/profile/avatar/upload",
    tag = "auth",
    operation_id = "upload_avatar",
    request_body(content = String, content_type = "multipart/form-data", description = "Avatar image to upload"),
    responses(
        (status = 200, description = "Avatar uploaded", body = inline(ApiResponse<UserProfileInfo>)),
        (status = 400, description = "Invalid image upload"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload_avatar(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    let profile = profile_service::upload_avatar(&state, claims.user_id, &mut payload).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/auth/profile/avatar/source",
    tag = "auth",
    operation_id = "set_avatar_source",
    request_body = UpdateAvatarSourceReq,
    responses(
        (status = 200, description = "Avatar source updated", body = inline(ApiResponse<UserProfileInfo>)),
        (status = 400, description = "Invalid avatar source"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn put_avatar_source(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdateAvatarSourceReq>,
) -> Result<HttpResponse> {
    let profile = profile_service::set_avatar_source(&state, claims.user_id, body.source).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/auth/profile/avatar/{size}",
    tag = "auth",
    operation_id = "get_self_avatar",
    params(("size" = u32, Path, description = "Avatar size (512 or 1024)")),
    responses(
        (status = 200, description = "Avatar image (WebP)"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Avatar not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_self_avatar(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<u32>,
) -> Result<HttpResponse> {
    let bytes = profile_service::get_avatar_bytes(&state, claims.user_id, *path).await?;
    Ok(profile_service::avatar_image_response(bytes))
}
