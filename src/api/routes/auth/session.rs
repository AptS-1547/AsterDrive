use super::{AuthTokenResp, ChangePasswordReq, storage_event_frame};
use crate::api::middleware::csrf::{self, RequestSourceMode};
use crate::api::request_auth::{access_cookie_token, bearer_token};
use crate::api::response::ApiResponse;
use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use crate::services::{audit_service, auth_service, team_service, user_service};
use actix_web::{HttpRequest, HttpResponse, web};
use bytes::Bytes;

use super::cookies::{
    REFRESH_COOKIE, build_access_cookie, build_csrf_cookie, build_refresh_cookie,
    clear_access_cookie, clear_csrf_cookie, clear_refresh_cookie,
};

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
                                &crate::services::storage_change_service::StorageChangeEvent::sync_required(),
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
    path = "/api/v1/auth/login",
    tag = "auth",
    operation_id = "login",
    request_body = super::LoginReq,
    responses(
        (status = 200, description = "Login successful, tokens set in HttpOnly cookies", body = inline(ApiResponse<AuthTokenResp>)),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<super::LoginReq>,
) -> Result<HttpResponse> {
    csrf::ensure_request_source_allowed(
        &req,
        &state.runtime_config,
        RequestSourceMode::OptionalWhenPresent,
    )?;
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
    let csrf_token = csrf::build_csrf_token();
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
        .cookie(build_csrf_cookie(
            &csrf_token,
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
pub async fn refresh(state: web::Data<AppState>, req: HttpRequest) -> Result<HttpResponse> {
    csrf::ensure_request_source_allowed(
        &req,
        &state.runtime_config,
        RequestSourceMode::OptionalWhenPresent,
    )?;
    csrf::ensure_double_submit_token(&req)?;
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let refresh_tok = req
        .cookie(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AsterError::auth_token_invalid("missing refresh cookie"))?;

    let access = auth_service::refresh_token(&state, &refresh_tok).await?;

    let secure = auth_policy.cookie_secure;
    let csrf_token = csrf::build_csrf_token();
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(
            &access,
            auth_policy.access_token_ttl_secs as i64,
            secure,
        ))
        .cookie(build_csrf_cookie(
            &csrf_token,
            auth_policy.refresh_token_ttl_secs as i64,
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
pub async fn logout(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if access_cookie_token(&req).is_some() || req.cookie(REFRESH_COOKIE).is_some() {
        if let Err(error) = csrf::ensure_request_source_allowed(
            &req,
            &state.runtime_config,
            RequestSourceMode::OptionalWhenPresent,
        ) {
            return actix_web::ResponseError::error_response(&error);
        }
        if let Err(error) = csrf::ensure_double_submit_token(&req) {
            return actix_web::ResponseError::error_response(&error);
        }
    }

    for token in [
        req.cookie(REFRESH_COOKIE)
            .map(|cookie| cookie.value().to_string()),
        access_cookie_token(&req),
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
        .cookie(clear_csrf_cookie(secure))
        .json(ApiResponse::<()>::ok_empty())
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    operation_id = "me",
    responses(
        (status = 200, description = "Current user info", body = inline(ApiResponse<crate::api::routes::auth::MeResponse>)),
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
    req: HttpRequest,
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
    let csrf_token = csrf::build_csrf_token();
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
        .cookie(build_csrf_cookie(
            &csrf_token,
            auth_policy.refresh_token_ttl_secs as i64,
            secure,
        ))
        .json(ApiResponse::ok(AuthTokenResp {
            expires_in: auth_policy.access_token_ttl_secs,
        })))
}
