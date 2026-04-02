#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::response::ApiResponse;
use crate::api::routes::batch;
use crate::errors::Result;
use crate::runtime::AppState;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::services::batch_service;
use crate::services::{auth_service::Claims, workspace_storage_service::WorkspaceStorageScope};
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/batch")
        .route("/delete", web::post().to(batch_delete))
        .route("/move", web::post().to(batch_move))
        .route("/copy", web::post().to(batch_copy))
}

fn team_scope(team_id: i64, user_id: i64) -> WorkspaceStorageScope {
    WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    }
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/delete",
    tag = "teams",
    operation_id = "batch_delete_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::batch::BatchDeleteReq,
    responses(
        (status = 200, description = "Team batch delete result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_delete(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::batch::BatchDeleteReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch::batch_delete_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/move",
    tag = "teams",
    operation_id = "batch_move_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::batch::BatchMoveReq,
    responses(
        (status = 200, description = "Team batch move result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_move(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::batch::BatchMoveReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch::batch_move_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/copy",
    tag = "teams",
    operation_id = "batch_copy_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::batch::BatchCopyReq,
    responses(
        (status = 200, description = "Team batch copy result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_copy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::batch::BatchCopyReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch::batch_copy_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        &body,
    )
    .await
}
