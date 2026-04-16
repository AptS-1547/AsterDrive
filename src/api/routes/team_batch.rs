use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::AuditContext, auth_service::Claims, batch_service, stream_ticket_service,
    task_service,
};
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
    let ctx = AuditContext::from_request(&req, &claims);
    let result = batch_service::batch_delete_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        &body.file_ids,
        &body.folder_ids,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
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
    let ctx = AuditContext::from_request(&req, &claims);
    let result = batch_service::batch_move_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
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
    let ctx = AuditContext::from_request(&req, &claims);
    let result = batch_service::batch_copy_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/archive-download",
    tag = "teams",
    operation_id = "batch_archive_download_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::batch::ArchiveDownloadReq,
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
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::batch::ArchiveDownloadReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch_service::validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let ticket = stream_ticket_service::create_archive_download_ticket_in_scope(
        &state,
        team_scope(team_id, claims.user_id),
        &task_service::CreateArchiveTaskParams {
            file_ids: body.file_ids,
            folder_ids: body.folder_ids,
            archive_name: body.archive_name,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(ticket)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/batch/archive-compress",
    tag = "teams",
    operation_id = "batch_archive_compress_team",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::batch::ArchiveCompressReq,
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
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::batch::ArchiveCompressReq>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let body = body.into_inner();
    batch_service::validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let task = task_service::create_archive_compress_task_in_scope(
        &state,
        team_scope(team_id, claims.user_id),
        task_service::CreateArchiveCompressTaskParams {
            file_ids: body.file_ids,
            folder_ids: body.folder_ids,
            archive_name: body.archive_name,
            target_folder_id: body.target_folder_id,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
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
    let scope = team_scope(team_id, claims.user_id);
    let params =
        stream_ticket_service::resolve_archive_download_ticket_in_scope(&state, scope, &token)
            .await?;
    task_service::stream_archive_download_in_scope(&state, scope, params).await
}
