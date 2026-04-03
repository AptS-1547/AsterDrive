use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::{ApiResponse, RemovedCountResponse};
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::lock_service;
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/locks",
    tag = "admin",
    operation_id = "list_locks",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "All WebDAV locks", body = inline(ApiResponse<OffsetPage<crate::entities::resource_lock::Model>>)),
        (status = 403, description = "Admin required"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_locks(
    state: web::Data<AppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let locks =
        lock_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(locks)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/locks/{id}",
    tag = "admin",
    operation_id = "force_unlock",
    params(("id" = i64, Path, description = "Lock ID")),
    responses(
        (status = 200, description = "Lock released"),
        (status = 403, description = "Admin required"),
        (status = 404, description = "Lock not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn force_unlock(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    lock_service::force_unlock(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/locks/expired",
    tag = "admin",
    operation_id = "cleanup_expired_locks",
    responses(
        (status = 200, description = "Expired locks cleaned up", body = inline(ApiResponse<crate::api::response::RemovedCountResponse>)),
        (status = 403, description = "Admin required"),
    ),
    security(("bearer" = [])),
)]
pub async fn cleanup_expired_locks(state: web::Data<AppState>) -> Result<HttpResponse> {
    let count = lock_service::cleanup_expired(&state).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(RemovedCountResponse { removed: count })))
}
