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
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::middleware::rate_limit;
use crate::config::RateLimitConfig;

const ACCESS_COOKIE: &str = "aster_access";
const REFRESH_COOKIE: &str = "aster_refresh";

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.auth);

    web::scope("/auth")
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/check", web::post().to(check))
        .route("/register", web::post().to(register))
        .route("/setup", web::post().to(setup))
        .route("/login", web::post().to(login))
        .route("/refresh", web::post().to(refresh))
        .route("/logout", web::post().to(logout))
        .route("/me", web::get().to(me))
        .route("/preferences", web::patch().to(patch_preferences))
}

#[derive(Deserialize, Serialize, ToSchema, Default, Clone)]
pub struct UserPreferences {
    pub theme_mode: Option<String>,
    pub color_preset: Option<String>,
    pub view_mode: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub language: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdatePreferencesReq {
    pub theme_mode: Option<String>,
    pub color_preset: Option<String>,
    pub view_mode: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub language: Option<String>,
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

/// Extract and verify bearer/cookie token, returning user_id
fn extract_user_id(
    req: &actix_web::HttpRequest,
    jwt_secret: &str,
) -> crate::errors::Result<i64> {
    let token = req
        .cookie(ACCESS_COOKIE)
        .map(|c| c.value().to_string())
        .or_else(|| {
            req.headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        })
        .ok_or_else(|| crate::errors::AsterError::auth_token_invalid("not authenticated"))?;
    let claims = auth_service::verify_token(&token, jwt_secret)?;
    Ok(claims.user_id)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    operation_id = "me",
    responses(
        (status = 200, description = "Current user info", body = inline(ApiResponse<crate::entities::user::Model>)),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn me(state: web::Data<AppState>, req: actix_web::HttpRequest) -> Result<HttpResponse> {
    let user_id = extract_user_id(&req, &state.config.auth.jwt_secret)?;
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let prefs: Option<UserPreferences> = user
        .config
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    // Build a combined response: all user fields + parsed preferences
    let resp = serde_json::json!({
        "id": user.id,
        "username": user.username,
        "email": user.email,
        "role": user.role,
        "status": user.status,
        "storage_used": user.storage_used,
        "storage_quota": user.storage_quota,
        "created_at": user.created_at,
        "updated_at": user.updated_at,
        "preferences": prefs,
    });
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

pub async fn patch_preferences(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    body: web::Json<UpdatePreferencesReq>,
) -> Result<HttpResponse> {
    let user_id = extract_user_id(&req, &state.config.auth.jwt_secret)?;
    let user = user_repo::find_by_id(&state.db, user_id).await?;

    // Deserialize existing prefs or start from default
    let mut prefs: UserPreferences = user
        .config
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Merge incoming fields
    if body.theme_mode.is_some() { prefs.theme_mode = body.theme_mode.clone(); }
    if body.color_preset.is_some() { prefs.color_preset = body.color_preset.clone(); }
    if body.view_mode.is_some() { prefs.view_mode = body.view_mode.clone(); }
    if body.sort_by.is_some() { prefs.sort_by = body.sort_by.clone(); }
    if body.sort_order.is_some() { prefs.sort_order = body.sort_order.clone(); }
    if body.language.is_some() { prefs.language = body.language.clone(); }

    let json_str = serde_json::to_string(&prefs)
        .map_err(|e| crate::errors::AsterError::internal_error(e.to_string()))?;

    let now = chrono::Utc::now();
    let mut active = user.into_active_model();
    active.config = ActiveValue::Set(Some(json_str));
    active.updated_at = ActiveValue::Set(now);
    active.save(&state.db).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(prefs)))
}
