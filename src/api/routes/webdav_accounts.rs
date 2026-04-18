//! API 路由：`webdav_accounts`。

use crate::api::dto::webdav::{CreateWebdavAccountReq, TestConnectionReq, WebdavSettingsInfo};
use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::config::site_url;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, webdav_account_service};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/webdav-accounts")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("", web::get().to(list_accounts))
        .route("", web::post().to(create_account))
        .route("/{id}", web::delete().to(delete_account))
        .route("/{id}/toggle", web::post().to(toggle_account))
        .route("/settings", web::get().to(get_settings))
        .route("/test", web::post().to(test_connection))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/webdav-accounts/settings",
    tag = "webdav",
    operation_id = "get_webdav_settings",
    responses(
        (status = 200, description = "Current WebDAV settings for the signed-in user", body = inline(ApiResponse<WebdavSettingsInfo>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_settings(state: web::Data<AppState>) -> Result<HttpResponse> {
    let endpoint_path = if state.config.webdav.prefix == "/" {
        "/".to_string()
    } else {
        format!("{}/", state.config.webdav.prefix.trim_end_matches('/'))
    };

    Ok(HttpResponse::Ok().json(ApiResponse::ok(WebdavSettingsInfo {
        prefix: state.config.webdav.prefix.clone(),
        endpoint: site_url::public_app_url_or_path(&state.runtime_config, &endpoint_path),
    })))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/webdav-accounts",
    tag = "webdav",
    operation_id = "list_webdav_accounts",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "WebDAV accounts", body = inline(ApiResponse<OffsetPage<webdav_account_service::WebdavAccountInfo>>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_accounts(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let accounts = webdav_account_service::list_paginated(
        &state,
        claims.user_id,
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(accounts)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/webdav-accounts",
    tag = "webdav",
    operation_id = "create_webdav_account",
    request_body = CreateWebdavAccountReq,
    responses(
        (status = 201, description = "Account created (password shown once)", body = inline(ApiResponse<webdav_account_service::WebdavAccountCreated>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_account(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateWebdavAccountReq>,
) -> Result<HttpResponse> {
    let result = webdav_account_service::create(
        &state,
        claims.user_id,
        &body.username,
        body.password.as_deref(),
        body.root_folder_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(result)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/webdav-accounts/{id}",
    tag = "webdav",
    operation_id = "delete_webdav_account",
    params(("id" = i64, Path, description = "Account ID")),
    responses(
        (status = 200, description = "Account deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_account(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    webdav_account_service::delete(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/webdav-accounts/{id}/toggle",
    tag = "webdav",
    operation_id = "toggle_webdav_account",
    params(("id" = i64, Path, description = "Account ID")),
    responses(
        (status = 200, description = "Account toggled", body = inline(ApiResponse<webdav_account_service::WebdavAccount>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn toggle_account(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let account = webdav_account_service::toggle_active(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(account)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/webdav-accounts/test",
    tag = "webdav",
    operation_id = "test_webdav_connection",
    request_body = TestConnectionReq,
    responses(
        (status = 200, description = "Connection successful"),
        (status = 401, description = "Invalid credentials"),
    ),
    security(("bearer" = [])),
)]
pub async fn test_connection(
    state: web::Data<AppState>,
    body: web::Json<TestConnectionReq>,
) -> Result<HttpResponse> {
    webdav_account_service::test_credentials(&state, &body.username, &body.password).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
