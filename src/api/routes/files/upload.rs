pub use crate::api::dto::files::*;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::AuditContext, auth_service::Claims, upload_service,
    workspace_storage_service::WorkspaceStorageScope,
};
use actix_web::{HttpRequest, HttpResponse, web};

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/upload",
    tag = "files",
    operation_id = "upload_file",
    params(FileQuery),
    request_body(content = String, content_type = "multipart/form-data", description = "File to upload"),
    responses(
        (status = 201, description = "File uploaded", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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
    body: web::Bytes,
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

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/upload/{upload_id}/complete",
    tag = "files",
    operation_id = "complete_chunked_upload",
    params(("upload_id" = String, Path, description = "Upload session ID")),
    request_body(content = CompleteUploadReq, description = "Multipart completion data (optional, only for presigned_multipart mode)", content_type = "application/json"),
    responses(
        (status = 201, description = "File created", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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
        .and_then(|payload| payload.into_inner().parts)
        .map(|parts| {
            parts
                .into_iter()
                .map(|part| (part.part_number, part.etag))
                .collect()
        });
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

#[allow(clippy::too_many_arguments)]
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
    let ctx = AuditContext::from_request(req, claims);
    let file = upload_service::upload_in_scope_with_audit(
        state,
        scope,
        folder_id,
        relative_path,
        declared_size,
        payload,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}
