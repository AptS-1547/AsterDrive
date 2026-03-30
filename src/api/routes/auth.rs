use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service, profile_service};
use actix_governor::Governor;
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::middleware::rate_limit;

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

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.auth);

    // 公开路由 + 认证路由分别注册到 /auth 路径下
    web::scope("/auth")
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/check", web::post().to(check))
        .route("/register", web::post().to(register))
        .route("/setup", web::post().to(setup))
        .route("/login", web::post().to(login))
        .route("/refresh", web::post().to(refresh))
        .route("/logout", web::post().to(logout))
        // 需要认证的端点使用嵌套 scope，注意路径前缀不能重复
        .service(
            web::scope("")
                .wrap(crate::api::middleware::auth::JwtAuth)
                .route("/me", web::get().to(me))
                .route("/password", web::put().to(put_password))
                .route("/preferences", web::patch().to(patch_preferences))
                .route("/profile", web::patch().to(patch_profile))
                .route("/profile/avatar/upload", web::post().to(upload_avatar))
                .route("/profile/avatar/source", web::put().to(put_avatar_source))
                .route("/profile/avatar/{size}", web::get().to(get_self_avatar)),
        )
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CheckReq {
    pub identifier: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct CheckResp {
    pub exists: bool,
    pub has_users: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct SetupReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginReq {
    pub identifier: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateAvatarSourceReq {
    pub source: AvatarSource,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateProfileReq {
    pub display_name: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

/// 构建 HttpOnly cookie
fn build_cookie(name: &str, value: &str, max_age_secs: i64, secure: bool) -> Cookie<'static> {
    Cookie::build(name.to_string(), value.to_string())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(CookieDuration::seconds(max_age_secs))
        .finish()
}

/// 构建清除 cookie
fn clear_cookie(name: &str, secure: bool) -> Cookie<'static> {
    Cookie::build(name.to_string(), "")
        .path("/")
        .http_only(true)
        .secure(secure)
        .max_age(CookieDuration::ZERO)
        .finish()
}

fn bearer_token(req: &actix_web::HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/check",
    tag = "auth",
    operation_id = "check_identifier",
    request_body = CheckReq,
    responses(
        (status = 200, description = "Check result", body = inline(ApiResponse<CheckResp>)),
    ),
)]
pub async fn check(state: web::Data<AppState>, body: web::Json<CheckReq>) -> Result<HttpResponse> {
    let (exists, has_users) = auth_service::check_identifier(&state, &body.identifier).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(CheckResp { exists, has_users })))
}

#[utoipa::path(
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
    let user_info =
        user_service::to_user_info(&state, &user, profile_service::AvatarAudience::SelfUser)
            .await?;
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

#[utoipa::path(
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
    let user_info =
        user_service::to_user_info(&state, &user, profile_service::AvatarAudience::SelfUser)
            .await?;
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    operation_id = "login",
    request_body = LoginReq,
    responses(
        (status = 200, description = "Login successful, tokens set in HttpOnly cookies"),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    body: web::Json<LoginReq>,
) -> Result<HttpResponse> {
    let result = auth_service::login(&state, &body.identifier, &body.password).await?;

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

    let secure = state.config.auth.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &result.access_token,
            state.config.auth.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_cookie(
            REFRESH_COOKIE,
            &result.refresh_token,
            state.config.auth.refresh_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    operation_id = "refresh",
    responses(
        (status = 200, description = "Token refreshed, new access token set in HttpOnly cookie"),
        (status = 401, description = "Invalid refresh token"),
    ),
)]
pub async fn refresh(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let refresh_tok = req
        .cookie(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| crate::errors::AsterError::auth_token_invalid("missing refresh cookie"))?;

    let access = auth_service::refresh_token(&state, &refresh_tok).await?;

    let secure = state.config.auth.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &access,
            state.config.auth.access_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
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

    let secure = state.config.auth.cookie_secure;
    HttpResponse::Ok()
        .cookie(clear_cookie(ACCESS_COOKIE, secure))
        .cookie(clear_cookie(REFRESH_COOKIE, secure))
        .json(ApiResponse::<()>::ok_empty())
}

#[utoipa::path(
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
    let resp = user_service::get_me(&state, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[utoipa::path(
    put,
    path = "/api/v1/auth/password",
    tag = "auth",
    operation_id = "change_password",
    request_body = ChangePasswordReq,
    responses(
        (status = 200, description = "Password updated"),
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
    let (access_token, refresh_token) =
        auth_service::issue_tokens_for_user(&user, &state.config.auth)?;

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

    let secure = state.config.auth.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &access_token,
            state.config.auth.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_cookie(
            REFRESH_COOKIE,
            &refresh_token,
            state.config.auth.refresh_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::<()>::ok_empty()))
}

/// Update the current user's preferences.
///
/// Only non-null fields in the request body are merged into the existing
/// preferences. Returns the full updated preferences object.
#[utoipa::path(
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

#[utoipa::path(
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

#[utoipa::path(
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

#[utoipa::path(
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

#[utoipa::path(
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
