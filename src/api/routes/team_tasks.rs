use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, task_service};
use actix_web::{HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/tasks")
        .route("", web::get().to(list_tasks))
        .route("/{id}", web::get().to(get_task))
        .route("/{id}/retry", web::post().to(retry_task))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/tasks",
    tag = "teams",
    operation_id = "list_team_tasks",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        LimitOffsetQuery
    ),
    responses(
        (status = 200, description = "Team tasks", body = inline(ApiResponse<OffsetPage<crate::services::task_service::TaskInfo>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_tasks(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<LimitOffsetQuery>,
) -> Result<HttpResponse> {
    let page = task_service::list_tasks_paginated_in_scope(
        &state,
        team_scope(*path, claims.user_id),
        query.limit_or(20, 100),
        query.offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/tasks/{id}",
    tag = "teams",
    operation_id = "get_team_task",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Team task details", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_task(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, task_id) = path.into_inner();
    let task =
        task_service::get_task_in_scope(&state, team_scope(team_id, claims.user_id), task_id)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/tasks/{id}/retry",
    tag = "teams",
    operation_id = "retry_team_task",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Team task reset for retry", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 400, description = "Task is not retryable"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn retry_task(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, task_id) = path.into_inner();
    let task =
        task_service::retry_task_in_scope(&state, team_scope(team_id, claims.user_id), task_id)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
}
