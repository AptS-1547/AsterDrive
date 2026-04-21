//! 管理员 API 路由：`shares`。

use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::share_service;
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/shares",
    tag = "admin",
    operation_id = "list_all_shares",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "All shares", body = inline(ApiResponse<OffsetPage<share_service::ShareInfo>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_all_shares(
    state: web::Data<PrimaryAppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let shares =
        share_service::list_paginated(&state, query.limit_or(50, 100), query.offset()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(shares)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/admin/shares/{id}",
    tag = "admin",
    operation_id = "admin_delete_share",
    params(("id" = i64, Path, description = "Share ID")),
    responses(
        (status = 200, description = "Share deleted"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Share not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn admin_delete_share(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    share_service::admin_delete_share(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
