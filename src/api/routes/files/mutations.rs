use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::AuditContext,
    auth_service::Claims,
    file_service,
    workspace_models::FileInfo,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::NullablePatch;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateEmptyRequest {
    pub name: String,
    pub folder_id: Option<i64>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ExtractArchiveRequest {
    pub target_folder_id: Option<i64>,
    pub output_folder_name: Option<String>,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/files/new",
    tag = "files",
    operation_id = "create_empty_file",
    request_body(content = CreateEmptyRequest, content_type = "application/json"),
    responses(
        (status = 201, description = "Empty file created", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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
    post,
    path = "/api/v1/files/{id}/extract",
    tag = "files",
    operation_id = "extract_file_archive",
    params(("id" = i64, Path, description = "File ID")),
    request_body = ExtractArchiveRequest,
    responses(
        (status = 200, description = "Archive extract task created", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 400, description = "Unsupported archive format"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn extract_archive(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<ExtractArchiveRequest>,
) -> Result<HttpResponse> {
    extract_archive_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        *path,
        &body,
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
        (status = 200, description = "File updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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

#[api_docs_macros::path(
    put,
    path = "/api/v1/files/{id}/content",
    tag = "files",
    operation_id = "update_file_content",
    params(("id" = i64, Path, description = "File ID")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Content updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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
        (status = 201, description = "File copied", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
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

pub(crate) async fn create_empty_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    body: &CreateEmptyRequest,
) -> Result<HttpResponse> {
    let file =
        workspace_storage_service::create_empty(state, scope, body.folder_id, &body.name).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(FileInfo::from(file))))
}

pub(crate) async fn extract_archive_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: &ExtractArchiveRequest,
) -> Result<HttpResponse> {
    let task = crate::services::task_service::create_archive_extract_task_in_scope(
        state,
        scope,
        file_id,
        crate::services::task_service::CreateArchiveExtractTaskParams {
            target_folder_id: body.target_folder_id,
            output_folder_name: body.output_folder_name.clone(),
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
}

pub(crate) async fn delete_file_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    file_service::delete_in_scope_with_audit(state, scope, file_id, &ctx).await?;
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
    let ctx = AuditContext::from_request(req, claims);
    let file = file_service::update_in_scope_with_audit(
        state,
        scope,
        file_id,
        body.name.clone(),
        body.folder_id,
        &ctx,
    )
    .await?;
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
    let if_match = req
        .headers()
        .get("If-Match")
        .and_then(|value| value.to_str().ok());
    let ctx = AuditContext::from_request(req, claims);
    let (file, new_hash) = file_service::update_content_in_scope_with_audit(
        state, scope, file_id, body, if_match, &ctx,
    )
    .await?;

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
    Ok(HttpResponse::Ok().json(ApiResponse::ok(FileInfo::from(file))))
}

pub(crate) async fn copy_file_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    file_id: i64,
    body: &CopyFileReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let file =
        file_service::copy_file_in_scope_with_audit(state, scope, file_id, body.folder_id, &ctx)
            .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}
