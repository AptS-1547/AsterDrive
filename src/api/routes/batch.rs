use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::AuditContext, auth_service::Claims, batch_service, stream_ticket_service,
    task_service, workspace_storage_service::WorkspaceStorageScope,
};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.write);

    web::scope("/batch")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
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

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchDeleteReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchMoveReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// 目标文件夹 ID（null = 根目录）
    pub target_folder_id: Option<i64>,
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchCopyReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// 目标文件夹 ID（null = 根目录）
    pub target_folder_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveDownloadReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveCompressReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
    pub target_folder_id: Option<i64>,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/batch/delete",
    tag = "batch",
    operation_id = "batch_delete",
    request_body = BatchDeleteReq,
    responses(
        (status = 200, description = "Batch delete result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_delete(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<BatchDeleteReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    batch_delete_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/batch/move",
    tag = "batch",
    operation_id = "batch_move",
    request_body = BatchMoveReq,
    responses(
        (status = 200, description = "Batch move result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_move(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<BatchMoveReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    batch_move_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/batch/copy",
    tag = "batch",
    operation_id = "batch_copy",
    request_body = BatchCopyReq,
    responses(
        (status = 200, description = "Batch copy result", body = inline(ApiResponse<batch_service::BatchResult>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn batch_copy(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<BatchCopyReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    batch_copy_response(
        &state,
        &claims,
        &req,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/batch/archive-download",
    tag = "batch",
    operation_id = "batch_archive_download",
    request_body = ArchiveDownloadReq,
    responses(
        (status = 200, description = "Archive download ticket", body = inline(ApiResponse<stream_ticket_service::StreamTicketInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<ArchiveDownloadReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    archive_download_ticket_response(
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
    path = "/api/v1/batch/archive-compress",
    tag = "batch",
    operation_id = "batch_archive_compress",
    request_body = ArchiveCompressReq,
    responses(
        (status = 200, description = "Archive compress task created", body = inline(ApiResponse<task_service::TaskInfo>)),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_compress(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<ArchiveCompressReq>,
) -> Result<HttpResponse> {
    let body = body.into_inner();
    archive_compress_response(
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
    path = "/api/v1/batch/archive-download/{token}",
    tag = "batch",
    operation_id = "batch_archive_download_stream",
    params(("token" = String, Path, description = "Archive download ticket")),
    responses(
        (status = 200, description = "Archive stream download"),
        (status = 400, description = "Invalid ticket"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn archive_download_stream(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let token = path.into_inner();
    archive_download_stream_response(
        &state,
        WorkspaceStorageScope::Personal {
            user_id: claims.user_id,
        },
        &token,
    )
    .await
}

pub(crate) async fn batch_delete_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    body: &BatchDeleteReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let result = batch_service::batch_delete_in_scope_with_audit(
        state,
        scope,
        &body.file_ids,
        &body.folder_ids,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

pub(crate) async fn batch_move_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    body: &BatchMoveReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let result = batch_service::batch_move_in_scope_with_audit(
        state,
        scope,
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

pub(crate) async fn batch_copy_response(
    state: &AppState,
    claims: &Claims,
    req: &HttpRequest,
    scope: WorkspaceStorageScope,
    body: &BatchCopyReq,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(req, claims);
    let result = batch_service::batch_copy_in_scope_with_audit(
        state,
        scope,
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

pub(crate) async fn archive_download_ticket_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    body: &ArchiveDownloadReq,
) -> Result<HttpResponse> {
    batch_service::validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let ticket = stream_ticket_service::create_archive_download_ticket_in_scope(
        state,
        scope,
        &task_service::CreateArchiveTaskParams {
            file_ids: body.file_ids.clone(),
            folder_ids: body.folder_ids.clone(),
            archive_name: body.archive_name.clone(),
        },
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::ok(ticket)))
}

pub(crate) async fn archive_compress_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    body: &ArchiveCompressReq,
) -> Result<HttpResponse> {
    batch_service::validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let task = task_service::create_archive_compress_task_in_scope(
        state,
        scope,
        task_service::CreateArchiveCompressTaskParams {
            file_ids: body.file_ids.clone(),
            folder_ids: body.folder_ids.clone(),
            archive_name: body.archive_name.clone(),
            target_folder_id: body.target_folder_id,
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(task)))
}

pub(crate) async fn archive_download_stream_response(
    state: &AppState,
    scope: WorkspaceStorageScope,
    token: &str,
) -> Result<HttpResponse> {
    let params =
        stream_ticket_service::resolve_archive_download_ticket_in_scope(state, scope, token)
            .await?;
    task_service::stream_archive_download_in_scope(state, scope, params).await
}
