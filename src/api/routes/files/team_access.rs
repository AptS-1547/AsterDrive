use super::{
    OpenWopiRequest, direct_link_response, download_response, get_file_response,
    get_thumbnail_response, open_wopi_response, preview_link_response,
};
#[cfg(all(feature = "openapi", debug_assertions))]
use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use actix_web::{HttpRequest, HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "get_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file info", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    get_file_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/direct-link",
    tag = "teams",
    operation_id = "get_team_file_direct_link",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file direct link token", body = inline(ApiResponse<crate::services::direct_link_service::DirectLinkTokenInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_direct_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    direct_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/preview-link",
    tag = "teams",
    operation_id = "create_team_file_preview_link",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file preview link", body = inline(ApiResponse<crate::services::preview_link_service::PreviewLinkInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_preview_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    preview_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/wopi/open",
    tag = "teams",
    operation_id = "open_team_file_with_wopi",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = OpenWopiRequest,
    responses(
        (status = 200, description = "Team WOPI launch session", body = inline(ApiResponse<crate::services::wopi_service::WopiLaunchSession>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_open_wopi(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<OpenWopiRequest>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    open_wopi_response(
        &state,
        team_scope(team_id, claims.user_id),
        file_id,
        &body.app_key,
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/thumbnail",
    tag = "teams",
    operation_id = "get_team_thumbnail",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 304, description = "Thumbnail not modified"),
        (status = 202, description = "Thumbnail generation in progress"),
        (status = 400, description = "Thumbnail not supported for this file type"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
        (status = 500, description = "Thumbnail generation failed"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_thumbnail(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    get_thumbnail_response(&state, &req, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/download",
    tag = "teams",
    operation_id = "download_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file content"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    download_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
    )
    .await
}
