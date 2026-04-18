//! 管理员 API 路由：`tasks`。

use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::task_service;
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/tasks",
    tag = "admin",
    operation_id = "admin_list_tasks",
    params(LimitOffsetQuery),
    responses(
        (status = 200, description = "All background tasks", body = inline(ApiResponse<OffsetPage<task_service::TaskInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_tasks(
    state: web::Data<AppState>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let page = task_service::list_tasks_paginated_for_admin(
        &state,
        query.limit_or(20, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}
