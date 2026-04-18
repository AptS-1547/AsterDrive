use super::{
    CopyFileReq, CreateEmptyRequest, ExtractArchiveRequest, PatchFileReq, SetLockReq,
    copy_file_response, create_empty_response, delete_file_response, extract_archive_response,
    patch_file_response, set_lock_response, update_content_response,
};
#[cfg(all(feature = "openapi", debug_assertions))]
use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use actix_web::{HttpRequest, HttpResponse, web};

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/new",
    tag = "teams",
    operation_id = "create_empty_team_file",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = CreateEmptyRequest,
    responses(
        (status = 201, description = "Empty team file created", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_create_empty(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<CreateEmptyRequest>,
) -> Result<HttpResponse> {
    create_empty_response(&state, team_scope(*path, claims.user_id), &body).await
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/teams/{team_id}/files/{id}/content",
    tag = "teams",
    operation_id = "update_team_file_content",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Content updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
        (status = 412, description = "Precondition failed (ETag mismatch)"),
        (status = 423, description = "File is locked by another user"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_update_content(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    update_content_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/extract",
    tag = "teams",
    operation_id = "extract_team_file_archive",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = ExtractArchiveRequest,
    responses(
        (status = 200, description = "Team archive extract task created", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 400, description = "Unsupported archive format"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_extract_archive(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<ExtractArchiveRequest>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    extract_archive_response(&state, team_scope(team_id, claims.user_id), file_id, &body).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/lock",
    tag = "teams",
    operation_id = "set_team_file_lock",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_set_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<SetLockReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    set_lock_response(
        &state,
        team_scope(team_id, claims.user_id),
        file_id,
        body.locked,
    )
    .await
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "patch_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = PatchFileReq,
    responses(
        (status = 200, description = "Team file updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_patch_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<PatchFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    patch_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/copy",
    tag = "teams",
    operation_id = "copy_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Source file ID")
    ),
    request_body = CopyFileReq,
    responses(
        (status = 201, description = "Team file copied", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_copy_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<CopyFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    copy_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        &body,
    )
    .await
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "delete_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_delete_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    delete_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
    )
    .await
}
