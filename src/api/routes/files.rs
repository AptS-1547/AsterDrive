use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    auth_service::Claims,
    file_service, upload_service, version_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::NullablePatch;
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/files")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/upload", web::post().to(upload))
        .route("/new", web::post().to(create_empty))
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
        .route(
            "/upload/{upload_id}/presign-parts",
            web::post().to(presign_parts),
        )
        .route("/upload/{upload_id}", web::get().to(get_upload_progress))
        .route("/upload/{upload_id}", web::delete().to(cancel_upload))
        // standard file routes
        .route("/{id}", web::get().to(get_file))
        .route("/{id}/download", web::get().to(download))
        .route("/{id}/thumbnail", web::get().to(get_thumbnail))
        .route("/{id}/content", web::put().to(update_content))
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct FileQuery {
    pub folder_id: Option<i64>,
    pub relative_path: Option<String>,
    pub declared_size: Option<i64>,
}

#[api_docs_macros::path(
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
    req: HttpRequest,
    query: web::Query<FileQuery>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    upload_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        query.folder_id,
        query.relative_path.as_deref(),
        query.declared_size,
        &mut payload,
    )
    .await
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateEmptyRequest {
    pub name: String,
    pub folder_id: Option<i64>,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/new",
    tag = "files",
    operation_id = "create_empty_file",
    request_body(content = CreateEmptyRequest, content_type = "application/json"),
    responses(
        (status = 201, description = "Empty file created", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 400, description = "Invalid name"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_empty(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateEmptyRequest>,
) -> Result<HttpResponse> {
    create_empty_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

#[api_docs_macros::path(
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
    get_file_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
    )
    .await
}

#[api_docs_macros::path(
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
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    download_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/files/{id}/thumbnail",
    tag = "files",
    operation_id = "get_thumbnail",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 202, description = "Thumbnail generation in progress"),
        (status = 400, description = "Thumbnail not supported for this file type"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
        (status = 500, description = "Thumbnail generation failed"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_thumbnail(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    get_thumbnail_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
    )
    .await
}

#[api_docs_macros::path(
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
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    delete_file_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
    )
    .await
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchFileReq {
    pub name: Option<String>,
    #[serde(default)]
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<i64>))]
    pub folder_id: NullablePatch<i64>,
}

#[api_docs_macros::path(
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
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<PatchFileReq>,
) -> Result<HttpResponse> {
    patch_file_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        &body,
    )
    .await
}

// ── Chunked Upload ──────────────────────────────────────────────────

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct InitUploadReq {
    pub filename: String,
    pub total_size: i64,
    pub folder_id: Option<i64>,
    pub relative_path: Option<String>,
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

#[api_docs_macros::path(
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
        body.relative_path.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(resp)))
}

#[api_docs_macros::path(
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CompleteUploadReq {
    pub parts: Option<Vec<CompletedPartReq>>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CompletedPartReq {
    pub part_number: i32,
    pub etag: String,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/upload/{upload_id}/complete",
    tag = "files",
    operation_id = "complete_chunked_upload",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    request_body(content = CompleteUploadReq, description = "Multipart completion data (optional, only for presigned_multipart mode)", content_type = "application/json"),
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
    body: Option<web::Json<CompleteUploadReq>>,
) -> Result<HttpResponse> {
    let parts = body
        .and_then(|b| b.into_inner().parts)
        .map(|parts| parts.into_iter().map(|p| (p.part_number, p.etag)).collect());
    let file =
        upload_service::complete_upload(&state, &path.upload_id, claims.user_id, parts).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/files/upload/{upload_id}",
    tag = "files",
    operation_id = "get_upload_progress",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    responses(
        (status = 200, description = "Upload progress", body = ApiResponse<upload_service::UploadProgressResponse>),
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

#[api_docs_macros::path(
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

// ── Presign Parts (S3 Multipart) ────────────────────────────────────

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PresignPartsReq {
    pub part_numbers: Vec<i32>,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/upload/{upload_id}/presign-parts",
    tag = "files",
    operation_id = "presign_upload_parts",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    request_body = PresignPartsReq,
    responses(
        (status = 200, description = "Presigned URLs for each part", body = inline(ApiResponse<std::collections::HashMap<i32, String>>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn presign_parts(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<UploadIdPath>,
    body: web::Json<PresignPartsReq>,
) -> Result<HttpResponse> {
    let urls = upload_service::presign_parts(
        &state,
        &path.upload_id,
        claims.user_id,
        body.into_inner().part_numbers,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(urls)))
}

// ── Content (Edit) ──────────────────────────────────────────────────

#[api_docs_macros::path(
    put,
    path = "/api/v1/files/{id}/content",
    tag = "files",
    operation_id = "update_file_content",
    params(("id" = i64, Path, description = "File ID")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Content updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
        (status = 412, description = "Precondition failed (ETag mismatch)"),
        (status = 423, description = "File is locked by another user"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_content(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse> {
    update_content_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        body,
    )
    .await
}

// ── Lock ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetLockReq {
    pub locked: bool,
}

#[api_docs_macros::path(
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
    set_lock_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        body.locked,
    )
    .await
}

// ── Copy ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CopyFileReq {
    /// 目标文件夹 ID（null = 根目录）
    pub folder_id: Option<i64>,
}

#[api_docs_macros::path(
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
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<CopyFileReq>,
) -> Result<HttpResponse> {
    copy_file_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        &body,
    )
    .await
}

pub(crate) async fn upload_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
    payload: &mut actix_multipart::Multipart,
) -> Result<HttpResponse> {
    let file = workspace_storage_service::upload(
        state,
        scope,
        payload,
        folder_id,
        relative_path,
        declared_size,
    )
    .await?;
    let ctx = AuditContext::from_request(req, claims);
    audit_service::log(
        state,
        &ctx,
        audit_service::AuditAction::FileUpload,
        Some("file"),
        Some(file.id),
        Some(&file.name),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

pub(crate) async fn create_empty_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    body: &CreateEmptyRequest,
) -> Result<HttpResponse> {
    let file =
        workspace_storage_service::create_empty(state, scope, body.folder_id, &body.name).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

pub(crate) async fn get_file_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<HttpResponse> {
    let file = file_service::get_info_in_scope(state, scope, file_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

pub(crate) async fn download_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<HttpResponse> {
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|v| v.to_str().ok());
    let response = file_service::download_in_scope(state, scope, file_id, if_none_match).await?;
    let ctx = AuditContext::from_request(req, claims);
    audit_service::log(
        state,
        &ctx,
        audit_service::AuditAction::FileDownload,
        Some("file"),
        Some(file_id),
        None,
        None,
    )
    .await;
    Ok(response)
}

pub(crate) async fn get_thumbnail_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<HttpResponse> {
    match file_service::get_thumbnail_data_in_scope(state, scope, file_id).await? {
        Some(result) => Ok(HttpResponse::Ok()
            .content_type("image/webp")
            .insert_header(("Cache-Control", "private, max-age=0, must-revalidate"))
            .body(result.data)),
        None => Ok(HttpResponse::Accepted()
            .insert_header(("Retry-After", "2"))
            .json(ApiResponse::<()>::ok_empty())),
    }
}

pub(crate) async fn delete_file_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<HttpResponse> {
    file_service::delete_in_scope(state, scope, file_id).await?;
    let ctx = AuditContext::from_request(req, claims);
    audit_service::log(
        state,
        &ctx,
        audit_service::AuditAction::FileDelete,
        Some("file"),
        Some(file_id),
        None,
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

pub(crate) async fn patch_file_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: &PatchFileReq,
) -> Result<HttpResponse> {
    let file =
        file_service::update_in_scope(state, scope, file_id, body.name.clone(), body.folder_id)
            .await?;
    let ctx = AuditContext::from_request(req, claims);
    let action = if body.folder_id.is_present() {
        audit_service::AuditAction::FileMove
    } else {
        audit_service::AuditAction::FileRename
    };
    audit_service::log(
        state,
        &ctx,
        action,
        Some("file"),
        Some(file.id),
        Some(&file.name),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

pub(crate) async fn update_content_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let if_match = req.headers().get("If-Match").and_then(|v| v.to_str().ok());
    let (file, new_hash) =
        file_service::update_content_in_scope(state, scope, file_id, body, if_match).await?;

    let ctx = AuditContext::from_request(req, claims);
    audit_service::log(
        state,
        &ctx,
        audit_service::AuditAction::FileEdit,
        Some("file"),
        Some(file.id),
        Some(&file.name),
        None,
    )
    .await;

    Ok(HttpResponse::Ok()
        .insert_header(("ETag", format!("\"{new_hash}\"")))
        .json(ApiResponse::ok(file)))
}

pub(crate) async fn set_lock_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    locked: bool,
) -> Result<HttpResponse> {
    let file = file_service::set_lock_in_scope(state, scope, file_id, locked).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

pub(crate) async fn copy_file_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: &CopyFileReq,
) -> Result<HttpResponse> {
    let file = file_service::copy_file_in_scope(state, scope, file_id, body.folder_id).await?;
    let ctx = AuditContext::from_request(req, claims);
    audit_service::log(
        state,
        &ctx,
        audit_service::AuditAction::FileCopy,
        Some("file"),
        Some(file.id),
        Some(&file.name),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

// ── Versions ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct VersionPath {
    pub id: i64,
    pub version_id: i64,
}

#[api_docs_macros::path(
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
