use crate::api::response::ApiResponse;
use crate::db::repository::user_repo;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{audit_service, auth_service};
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

const ACCESS_COOKIE: &str = "aster_access";
const REFRESH_COOKIE: &str = "aster_refresh";

pub fn routes() -> actix_web::Scope {
    let login_limiter = GovernorConfigBuilder::default()
        .seconds_per_request(1)
        .burst_size(5)
        .finish()
        .unwrap();

    let register_limiter = GovernorConfigBuilder::default()
        .seconds_per_request(1)
        .burst_size(3)
        .finish()
        .unwrap();

    web::scope("/auth")
        .service(
            web::resource("/register")
                .wrap(Governor::new(&register_limiter))
                .route(web::post().to(register)),
        )
        .service(
            web::resource("/login")
                .wrap(Governor::new(&login_limiter))
                .route(web::post().to(login)),
        )
        .route("/refresh", web::post().to(refresh))
        .route("/logout", web::post().to(logout))
        .route("/me", web::get().to(me))
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

/// 构建 HttpOnly cookie
fn build_cookie(name: &str, value: &str, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build(name.to_string(), value.to_string())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(max_age_secs))
        .finish()
}

/// 构建清除 cookie
fn clear_cookie(name: &str) -> Cookie<'static> {
    Cookie::build(name.to_string(), "")
        .path("/")
        .http_only(true)
        .max_age(CookieDuration::ZERO)
        .finish()
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
    let (access, refresh_tok) = auth_service::login(&state, &body.username, &body.password).await?;

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
            Some(&body.username),
            None,
        )
        .await;
    }

    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &access,
            state.config.auth.access_token_ttl_secs as i64,
        ))
        .cookie(build_cookie(
            REFRESH_COOKIE,
            &refresh_tok,
            state.config.auth.refresh_token_ttl_secs as i64,
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

    Ok(HttpResponse::Ok()
        .cookie(build_cookie(
            ACCESS_COOKIE,
            &access,
            state.config.auth.access_token_ttl_secs as i64,
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
pub async fn logout() -> HttpResponse {
    HttpResponse::Ok()
        .cookie(clear_cookie(ACCESS_COOKIE))
        .cookie(clear_cookie(REFRESH_COOKIE))
        .json(ApiResponse::<()>::ok_empty())
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
    // 从 cookie 或 header 取 token
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

    let claims = auth_service::verify_token(&token, &state.config.auth.jwt_secret)?;
    let user = user_repo::find_by_id(&state.db, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user)))
}
