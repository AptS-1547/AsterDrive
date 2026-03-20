use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, file_service};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/files")
        .wrap(JwtAuth)
        .route("/upload", web::post().to(upload))
        .route("/{id}", web::get().to(get_file))
        .route("/{id}/download", web::get().to(download))
        .route("/{id}", web::delete().to(delete_file))
        .route("/{id}", web::patch().to(patch_file))
}

#[derive(Deserialize, IntoParams, ToSchema)]
pub struct FileQuery {
    pub folder_id: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/files/upload",
    tag = "files",
    operation_id = "upload_file",
    params(FileQuery),
    request_body(content = String, content_type = "multipart/form-data", description = "File to upload"),
    responses(
        (status = 201, description = "File uploaded", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<FileQuery>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    let file = file_service::upload(
        &state.db,
        &state.driver_registry,
        claims.user_id,
        &mut payload,
        query.folder_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}",
    tag = "files",
    operation_id = "get_file",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File info", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let file = file_service::get_info(&state.db, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}/download",
    tag = "files",
    operation_id = "download_file",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File content"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let response =
        file_service::download(&state.db, &state.driver_registry, *path, claims.user_id).await?;
    Ok(response)
}

#[utoipa::path(
    delete,
    path = "/api/v1/files/{id}",
    tag = "files",
    operation_id = "delete_file",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    file_service::delete(&state.db, &state.driver_registry, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[derive(Deserialize, ToSchema)]
pub struct PatchFileReq {
    pub name: Option<String>,
    pub folder_id: Option<i64>,
}

#[utoipa::path(
    patch,
    path = "/api/v1/files/{id}",
    tag = "files",
    operation_id = "patch_file",
    params(("id" = i64, Path, description = "File ID")),
    request_body = PatchFileReq,
    responses(
        (status = 200, description = "File updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<PatchFileReq>,
) -> Result<HttpResponse> {
    let file = file_service::update(
        &state.db,
        *path,
        claims.user_id,
        body.name.clone(),
        body.folder_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}
