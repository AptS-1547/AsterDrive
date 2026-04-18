pub use crate::api::dto::shares::*;
use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::services::batch_service;
use crate::services::{
    audit_service::AuditContext, auth_service::Claims, share_service,
    workspace_storage_service::WorkspaceStorageScope,
};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/shares")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("", web::post().to(create_share))
        .route("", web::get().to(list_shares))
        .route("/batch-delete", web::post().to(batch_delete_shares))
        .route("/{id}", web::patch().to(update_share))
        .route("/{id}", web::delete().to(delete_share))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/shares",
    tag = "shares",
    operation_id = "create_share",
    request_body = CreateShareReq,
    responses(
        (status = 201, description = "Share created", body = inline(ApiResponse<crate::services::share_service::ShareInfo>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<CreateShareReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    create_share_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/shares",
    tag = "shares",
    operation_id = "list_my_shares",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "My shares", body = inline(ApiResponse<OffsetPage<crate::services::share_service::MyShareInfo>>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    list_shares_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &query,
    )
    .await
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/shares/{id}",
    tag = "shares",
    operation_id = "update_share",
    params(("id" = i64, Path, description = "Share ID")),
    request_body = UpdateShareReq,
    responses(
        (status = 200, description = "Share updated", body = inline(ApiResponse<crate::services::share_service::ShareInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateShareReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    update_share_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        &body,
    )
    .await
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/shares/{id}",
    tag = "shares",
    operation_id = "delete_share",
    params(("id" = i64, Path, description = "Share ID")),
    responses(
        (status = 200, description = "Share deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    delete_share_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/shares/batch-delete",
    tag = "shares",
    operation_id = "batch_delete_shares",
    request_body = BatchDeleteSharesReq,
    responses(
        (status = 200, description = "Batch delete result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_delete_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<BatchDeleteSharesReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    batch_delete_shares_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

pub(crate) async fn create_share_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    body: &CreateShareReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let share = share_service::create_share_in_scope_with_audit(
        state,
        scope,
        body.target,
        body.password.clone(),
        body.expires_at,
        body.max_downloads,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(share)))
}

pub(crate) async fn list_shares_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    query: &LimitOffsetQuery,
) -> Result<HttpResponse> {
    let shares = share_service::list_shares_paginated_in_scope(
        state,
        scope,
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

pub(crate) async fn update_share_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    share_id: i64,
    body: &UpdateShareReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let share = share_service::update_share_in_scope_with_audit(
        state,
        scope,
        share_id,
        body.password.clone(),
        body.expires_at,
        body.max_downloads,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(share)))
}

pub(crate) async fn delete_share_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    share_id: i64,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    share_service::delete_share_in_scope_with_audit(state, scope, share_id, &ctx).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

pub(crate) async fn batch_delete_shares_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    body: &BatchDeleteSharesReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let result =
        share_service::batch_delete_shares_in_scope_with_audit(state, scope, &body.share_ids, &ctx)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}
