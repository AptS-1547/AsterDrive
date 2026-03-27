use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    auth_service::Claims,
    batch_service,
};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.write);

    web::scope("/batch")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/delete", web::post().to(batch_delete))
        .route("/move", web::post().to(batch_move))
        .route("/copy", web::post().to(batch_copy))
}

#[derive(Deserialize, ToSchema)]
pub struct BatchDeleteReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct BatchMoveReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// 目标文件夹 ID（null = 根目录）
    pub target_folder_id: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct BatchCopyReq {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    /// 目标文件夹 ID（null = 根目录）
    pub target_folder_id: Option<i64>,
}

const MAX_BATCH_ITEMS: usize = 1000;

fn validate_batch_ids(file_ids: &[i64], folder_ids: &[i64]) -> Result<()> {
    if file_ids.is_empty() && folder_ids.is_empty() {
        return Err(AsterError::validation_error(
            "at least one file or folder ID is required",
        ));
    }
    if file_ids.len() + folder_ids.len() > MAX_BATCH_ITEMS {
        return Err(AsterError::validation_error(format!(
            "batch size cannot exceed {MAX_BATCH_ITEMS} items",
        )));
    }
    Ok(())
}

#[utoipa::path(
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
    validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let result =
        batch_service::batch_delete(&state, claims.user_id, &body.file_ids, &body.folder_ids)
            .await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state, &ctx, "batch_delete", None, None, None,
        Some(serde_json::json!({ "file_ids": body.file_ids, "folder_ids": body.folder_ids, "succeeded": result.succeeded, "failed": result.failed })),
    ).await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

#[utoipa::path(
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
    validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let result = batch_service::batch_move(
        &state,
        claims.user_id,
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
    )
    .await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state, &ctx, "batch_move", None, None, None,
        Some(serde_json::json!({ "file_ids": body.file_ids, "folder_ids": body.folder_ids, "target_folder_id": body.target_folder_id, "succeeded": result.succeeded, "failed": result.failed })),
    ).await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}

#[utoipa::path(
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
    validate_batch_ids(&body.file_ids, &body.folder_ids)?;
    let result = batch_service::batch_copy(
        &state,
        claims.user_id,
        &body.file_ids,
        &body.folder_ids,
        body.target_folder_id,
    )
    .await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state, &ctx, "batch_copy", None, None, None,
        Some(serde_json::json!({ "file_ids": body.file_ids, "folder_ids": body.folder_ids, "target_folder_id": body.target_folder_id, "succeeded": result.succeeded, "failed": result.failed })),
    ).await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(result)))
}
