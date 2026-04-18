//! 文件 API 路由：`versions`。

use crate::api::dto::files::VersionPath;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, version_service};
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/files/{id}/versions",
    tag = "files",
    operation_id = "list_versions",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File versions", body = inline(ApiResponse<Vec<crate::services::workspace_models::FileVersion>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
    ),
    security(("bearer" = [])),
)]
pub async fn list_versions(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let versions = version_service::list_versions(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(versions)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/{id}/versions/{version_id}/restore",
    tag = "files",
    operation_id = "restore_version",
    params(
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version restored", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn restore_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<VersionPath>,
) -> Result<HttpResponse> {
    let file =
        version_service::restore_version(&state, path.id, path.version_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/files/{id}/versions/{version_id}",
    tag = "files",
    operation_id = "delete_version",
    params(
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version deleted"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<VersionPath>,
) -> Result<HttpResponse> {
    version_service::delete_version(&state, path.id, path.version_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/versions",
    tag = "teams",
    operation_id = "list_team_versions",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "File versions", body = inline(ApiResponse<Vec<crate::services::workspace_models::FileVersion>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_list_versions(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    let versions =
        version_service::list_versions_for_team(&state, team_id, file_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(versions)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/versions/{version_id}/restore",
    tag = "teams",
    operation_id = "restore_team_version",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version restored", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_restore_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    let file = version_service::restore_version_for_team(
        &state,
        team_id,
        file_id,
        version_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/files/{id}/versions/{version_id}",
    tag = "teams",
    operation_id = "delete_team_version",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version deleted"),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_delete_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    version_service::delete_version_for_team(&state, team_id, file_id, version_id, claims.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
