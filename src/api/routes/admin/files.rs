//! 管理员 API 路由：`files` / `file-blobs` observability。

use crate::api::dto::admin::{AdminFileBlobListQuery, AdminFileListQuery};
use crate::api::pagination::LimitOffsetQuery;
#[cfg(all(debug_assertions, feature = "openapi"))]
use crate::api::pagination::OffsetPage;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::admin_file_service;
use actix_web::{HttpResponse, web};

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/files",
    tag = "admin",
    operation_id = "admin_list_files",
    params(LimitOffsetQuery, AdminFileListQuery),
    responses(
        (status = 200, description = "List files with current blob summary", body = inline(ApiResponse<OffsetPage<crate::api::dto::admin::AdminFileInfo>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_files(
    state: web::Data<PrimaryAppState>,
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<AdminFileListQuery>,
) -> Result<HttpResponse> {
    let page =
        admin_file_service::list_files(&state, page.limit_or(50, 100), page.offset(), &query)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/files/{id}",
    tag = "admin",
    operation_id = "admin_get_file",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File details with current blob and version summaries", body = inline(ApiResponse<crate::api::dto::admin::AdminFileDetail>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = crate::api::constants::OPENAPI_NOT_FOUND),
    ),
    security(("bearer" = [])),
)]
pub async fn get_file(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let file = admin_file_service::get_file(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/file-blobs",
    tag = "admin",
    operation_id = "admin_list_file_blobs",
    params(LimitOffsetQuery, AdminFileBlobListQuery),
    responses(
        (status = 200, description = "List file blobs", body = inline(ApiResponse<OffsetPage<crate::api::dto::admin::AdminFileBlobInfo>>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_file_blobs(
    state: web::Data<PrimaryAppState>,
    page: web::Query<LimitOffsetQuery>,
    query: web::Query<AdminFileBlobListQuery>,
) -> Result<HttpResponse> {
    let page =
        admin_file_service::list_blobs(&state, page.limit_or(50, 100), page.offset(), &query)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(page)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/admin/file-blobs/{id}",
    tag = "admin",
    operation_id = "admin_get_file_blob",
    params(("id" = i64, Path, description = "File blob ID")),
    responses(
        (status = 200, description = "File blob details with file and version references", body = inline(ApiResponse<crate::api::dto::admin::AdminFileBlobDetail>)),
        (status = 401, description = crate::api::constants::OPENAPI_UNAUTHORIZED),
        (status = 403, description = "Forbidden"),
        (status = 404, description = crate::api::constants::OPENAPI_NOT_FOUND),
    ),
    security(("bearer" = [])),
)]
pub async fn get_file_blob(
    state: web::Data<PrimaryAppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let blob = admin_file_service::get_blob(&state, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(blob)))
}
