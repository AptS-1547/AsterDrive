use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::db::repository::file_repo;
use crate::errors::AsterError;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, file_service, thumbnail_service, upload_service};
use crate::types::EntityType;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/files")
        .wrap(JwtAuth)
        .route("/upload", web::post().to(upload))
        // chunked upload routes (before /{id} to avoid conflicts)
        .route("/upload/init", web::post().to(init_chunked_upload))
        .route(
            "/upload/{upload_id}/{chunk_number}",
            web::put().to(upload_chunk),
        )
        .route(
            "/upload/{upload_id}/complete",
            web::post().to(complete_upload),
        )
        .route("/upload/{upload_id}", web::get().to(get_upload_progress))
        .route("/upload/{upload_id}", web::delete().to(cancel_upload))
        // standard file routes
        .route("/{id}", web::get().to(get_file))
        .route("/{id}/download", web::get().to(download))
        .route("/{id}/thumbnail", web::get().to(get_thumbnail))
        .route("/{id}/lock", web::post().to(set_lock))
        .route("/{id}/copy", web::post().to(copy_file))
        .route("/{id}/versions", web::get().to(list_versions))
        .route(
            "/{id}/versions/{version_id}/restore",
            web::post().to(restore_version),
        )
        .route(
            "/{id}/versions/{version_id}",
            web::delete().to(delete_version),
        )
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
    let file = file_service::upload(&state, claims.user_id, &mut payload, query.folder_id).await?;
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
    let file = file_service::get_info(&state, *path, claims.user_id).await?;
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
    let response = file_service::download(&state, *path, claims.user_id).await?;
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}/thumbnail",
    tag = "files",
    operation_id = "get_thumbnail",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found or not an image"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_thumbnail(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let f = file_service::get_info(&state, *path, claims.user_id).await?;
    if !thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }
    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    let data = thumbnail_service::get_or_generate(&state, &blob).await?;
    Ok(HttpResponse::Ok()
        .content_type("image/webp")
        .insert_header(("Cache-Control", "public, max-age=31536000, immutable"))
        .body(data))
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
    file_service::delete(&state, *path, claims.user_id).await?;
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
        &state,
        *path,
        claims.user_id,
        body.name.clone(),
        body.folder_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

// ── Chunked Upload ──────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct InitUploadReq {
    pub filename: String,
    pub total_size: i64,
    pub folder_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct ChunkPath {
    pub upload_id: String,
    pub chunk_number: i32,
}

#[derive(Deserialize)]
pub struct UploadIdPath {
    pub upload_id: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/files/upload/init",
    tag = "files",
    operation_id = "init_chunked_upload",
    request_body = InitUploadReq,
    responses(
        (status = 201, description = "Upload session created", body = inline(ApiResponse<upload_service::InitUploadResponse>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn init_chunked_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<InitUploadReq>,
) -> Result<HttpResponse> {
    let resp = upload_service::init_upload(
        &state,
        claims.user_id,
        &body.filename,
        body.total_size,
        body.folder_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(resp)))
}

#[utoipa::path(
    put,
    path = "/api/v1/files/upload/{upload_id}/{chunk_number}",
    tag = "files",
    operation_id = "upload_chunk",
    params(
        ("upload_id" = String, Path, description = "Upload session ID"),
        ("chunk_number" = i32, Path, description = "Chunk number (0-indexed)"),
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Chunk uploaded", body = inline(ApiResponse<upload_service::ChunkUploadResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload_chunk(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<ChunkPath>,
    body: actix_web::web::Bytes,
) -> Result<HttpResponse> {
    let resp = upload_service::upload_chunk(
        &state,
        &path.upload_id,
        path.chunk_number,
        claims.user_id,
        &body,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[utoipa::path(
    post,
    path = "/api/v1/files/upload/{upload_id}/complete",
    tag = "files",
    operation_id = "complete_chunked_upload",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    responses(
        (status = 201, description = "File created", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn complete_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UploadIdPath>,
) -> Result<HttpResponse> {
    let file = upload_service::complete_upload(&state, &path.upload_id, claims.user_id).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

#[utoipa::path(
    get,
    path = "/api/v1/files/upload/{upload_id}",
    tag = "files",
    operation_id = "get_upload_progress",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    responses(
        (status = 200, description = "Upload progress", body = inline(ApiResponse<upload_service::UploadProgressResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_upload_progress(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UploadIdPath>,
) -> Result<HttpResponse> {
    let resp = upload_service::get_progress(&state, &path.upload_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/files/upload/{upload_id}",
    tag = "files",
    operation_id = "cancel_upload",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    responses(
        (status = 200, description = "Upload cancelled"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn cancel_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UploadIdPath>,
) -> Result<HttpResponse> {
    upload_service::cancel_upload(&state, &path.upload_id, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

// ── Lock ────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct SetLockReq {
    pub locked: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/files/{id}/lock",
    tag = "files",
    operation_id = "set_file_lock",
    params(("id" = i64, Path, description = "File ID")),
    request_body = SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<SetLockReq>,
) -> Result<HttpResponse> {
    use crate::services::lock_service;
    if body.locked {
        lock_service::lock(
            &state,
            EntityType::File,
            *path,
            Some(claims.user_id),
            None,
            None,
        )
        .await?;
    } else {
        lock_service::unlock(&state, EntityType::File, *path, claims.user_id).await?;
    }
    let file = file_service::get_info(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

// ── Copy ───────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CopyFileReq {
    pub folder_id: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/files/{id}/copy",
    tag = "files",
    operation_id = "copy_file",
    params(("id" = i64, Path, description = "Source file ID")),
    request_body = CopyFileReq,
    responses(
        (status = 201, description = "File copied", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn copy_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<CopyFileReq>,
) -> Result<HttpResponse> {
    let file = file_service::copy_file(&state, *path, claims.user_id, body.folder_id).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

// ── Versions ───────────────────────────────────────────────────────

use crate::services::version_service;

#[derive(Deserialize)]
pub struct VersionPath {
    pub id: i64,
    pub version_id: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}/versions",
    tag = "files",
    operation_id = "list_versions",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "File versions", body = inline(ApiResponse<Vec<crate::entities::file_version::Model>>)),
        (status = 401, description = "Unauthorized"),
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

#[utoipa::path(
    post,
    path = "/api/v1/files/{id}/versions/{version_id}/restore",
    tag = "files",
    operation_id = "restore_version",
    params(
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version restored", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
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

#[utoipa::path(
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
        (status = 401, description = "Unauthorized"),
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
