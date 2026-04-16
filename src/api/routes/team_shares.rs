use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::services::batch_service;
use crate::services::{audit_service::AuditContext, auth_service::Claims, share_service};
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/shares")
        .route("", web::post().to(create_share))
        .route("", web::get().to(list_shares))
        .route("/batch-delete", web::post().to(batch_delete_shares))
        .route("/{id}", web::patch().to(update_share))
        .route("/{id}", web::delete().to(delete_share))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/shares",
    tag = "teams",
    operation_id = "create_team_share",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::shares::CreateShareReq,
    responses(
        (status = 201, description = "Team share created", body = inline(ApiResponse<crate::services::share_service::ShareInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::shares::CreateShareReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    let share = share_service::create_share_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        body.target,
        body.password,
        body.expires_at,
        body.max_downloads,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(share)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/shares",
    tag = "teams",
    operation_id = "list_team_shares",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        LimitOffsetQuery
    ),
    responses(
        (status = 200, description = "Team shares", body = inline(ApiResponse<OffsetPage<crate::services::share_service::MyShareInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let shares = share_service::list_shares_paginated_in_scope(
        &state,
        team_scope(*path, claims.user_id),
        query.limit_or(50, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{team_id}/shares/{id}",
    tag = "teams",
    operation_id = "update_team_share",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Share ID")
    ),
    request_body = crate::api::routes::shares::UpdateShareReq,
    responses(
        (status = 200, description = "Team share updated", body = inline(ApiResponse<crate::services::share_service::ShareInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::shares::UpdateShareReq>,
) -> Result<HttpResponse> {
    let (team_id, share_id) = path.into_inner();
    let body = body.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    let share = share_service::update_share_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        share_id,
        body.password,
        body.expires_at,
        body.max_downloads,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(share)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/shares/{id}",
    tag = "teams",
    operation_id = "delete_team_share",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Share ID")
    ),
    responses(
        (status = 200, description = "Team share deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_share(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, share_id) = path.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    share_service::delete_share_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        share_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/shares/batch-delete",
    tag = "teams",
    operation_id = "batch_delete_team_shares",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::shares::BatchDeleteSharesReq,
    responses(
        (status = 200, description = "Batch delete result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_delete_shares(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::shares::BatchDeleteSharesReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    let result = share_service::batch_delete_shares_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        &body.share_ids,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}
