#[cfg(all(feature = "openapi", debug_assertions))]
use crate::api::response::ApiResponse;
use crate::api::routes::batch::{
    ArchiveCompressReq, ArchiveDownloadReq, BatchCopyReq, BatchDeleteReq, BatchMoveReq,
    archive_compress_response, archive_download_stream_response, archive_download_ticket_response,
    batch_copy_response, batch_delete_response, batch_move_response,
};
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/batch")
        .route("/delete", web::post().to(batch_delete))
        .route("/move", web::post().to(batch_move))
        .route("/copy", web::post().to(batch_copy))
        .route("/archive-compress", web::post().to(archive_compress))
        .route("/archive-download", web::post().to(archive_download))
        .route(
            "/archive-download/{token}",
            web::get().to(archive_download_stream),
        )
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/delete",
    tag = "teams",
    operation_id = "batch_delete_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = BatchDeleteReq,
    responses(
        (status = 200, description = "Team batch delete result", body = inline(ApiResponse<crate::services::batch_service::BatchResult>)),
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
    body: web::Json<BatchDeleteReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch_delete_response(
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
    request_body = BatchMoveReq,
    responses(
        (status = 200, description = "Team batch move result", body = inline(ApiResponse<crate::services::batch_service::BatchResult>)),
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
    body: web::Json<BatchMoveReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch_move_response(
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
    request_body = BatchCopyReq,
    responses(
        (status = 200, description = "Team batch copy result", body = inline(ApiResponse<crate::services::batch_service::BatchResult>)),
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
    body: web::Json<BatchCopyReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch_copy_response(
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
    path = "/api/v1/teams/{team_id}/batch/archive-download",
    tag = "teams",
    operation_id = "batch_archive_download_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = ArchiveDownloadReq,
    responses(
        (status = 200, description = "Team archive download ticket", body = inline(ApiResponse<crate::services::stream_ticket_service::StreamTicketInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_download(
    state: web::Data<AppState>,
    _claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<ArchiveDownloadReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    archive_download_ticket_response(&state, team_scope(team_id, _claims.user_id), &body).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/archive-compress",
    tag = "teams",
    operation_id = "batch_archive_compress_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = ArchiveCompressReq,
    responses(
        (status = 200, description = "Team archive compress task created", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_compress(
    state: web::Data<AppState>,
    _claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<ArchiveCompressReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    archive_compress_response(&state, team_scope(team_id, _claims.user_id), &body).await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/batch/archive-download/{token}",
    tag = "teams",
    operation_id = "batch_archive_download_stream_team",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("token" = String, Path, description = "Archive download ticket")
    ),
    responses(
        (status = 200, description = "Team archive stream download"),
        (status = 400, description = "Invalid ticket"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_download_stream(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
) -> Result<HttpResponse> {
    let (team_id, token) = path.into_inner();
    archive_download_stream_response(&state, team_scope(team_id, claims.user_id), &token).await
}
