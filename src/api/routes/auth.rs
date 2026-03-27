use crate::api::response::ApiResponse;
use crate::db::repository::user_repo;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service};
use actix_governor::Governor;
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::middleware::rate_limit;
use crate::config::RateLimitConfig;

// Re-export preference types from user_service for OpenAPI schema registration.
pub use crate::services::user_service::{
    ColorPreset, Language, PrefViewMode, ThemeMode, UpdatePreferencesReq, UserPreferences,
};

use crate::services::auth_service::Claims;
use crate::services::user_service::{parse_preferences, update_preferences};

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
                .route("/preferences", web::patch().to(patch_preferences)),
        )
}

/// 用户信息核心字段（不含 password_hash），用于 API 响应。
#[derive(Debug, Serialize, ToSchema)]
pub struct UserCore {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: crate::types::UserRole,
    pub status: crate::types::UserStatus,
    pub storage_used: i64,
    pub storage_quota: i64,
    #[schema(value_type = String)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// /auth/me 响应：用户信息 + 偏好设置。
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MeResponse)]
pub struct MeResponse {
    #[serde(flatten)]
    pub user: UserCore,
    pub preferences: Option<UserPreferences>,
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
        (status = 201, description = "Admin account created", body = inline(ApiResponse<crate::entities::user::Model>)),
        (status = 400, description = "System already initialized"),
    ),
)]
pub async fn setup(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    body: web::Json<SetupReq>,
) -> Result<HttpResponse> {
    let user = auth_service::setup(&state, &body.username, &body.email, &body.password).await?;
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
        "system_setup",
        None,
        None,
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    operation_id = "register",
    request_body = RegisterReq,
    responses(
        (status = 201, description = "Registration successful", body = inline(ApiResponse<crate::entities::user::Model>)),
        (status = 400, description = "Validation error"),
    ),
)]
pub async fn register(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    body: web::Json<RegisterReq>,
) -> Result<HttpResponse> {
    let user = auth_service::register(&state, &body.username, &body.email, &body.password).await?;
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
        "user_register",
        None,
        None,
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(user)))
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
    let (access, refresh_tok) =
        auth_service::login(&state, &body.identifier, &body.password).await?;

    // 审计日志 — 从 token 解析 user_id
    if let Ok(claims) = auth_service::verify_token(&access, &state.config.auth.jwt_secret) {
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
            "user_login",
            None,
            None,
            Some(&body.identifier),
            None,
        )
        .await;
    }

    let secure = state.config.auth.cookie_secure;
    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &access,
            state.config.auth.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_cookie(
            REFRESH_COOKIE,
            &refresh_tok,
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

    let access = auth_service::refresh_token(&state, &refresh_tok)?;

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
pub async fn logout(state: web::Data<AppState>) -> HttpResponse {
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
    let user = user_repo::find_by_id(&state.db, claims.user_id).await?;
    let prefs = parse_preferences(&user);
    let resp = MeResponse {
        user: UserCore {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user.role,
            status: user.status,
            storage_used: user.storage_used,
            storage_quota: user.storage_quota,
            created_at: user.created_at,
            updated_at: user.updated_at,
        },
        preferences: prefs,
    };
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
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
    let prefs = update_preferences(&state, claims.user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(prefs)))
}
