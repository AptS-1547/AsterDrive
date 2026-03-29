use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    auth_service::Claims,
    folder_service,
};
use crate::types::NullablePatch;
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/folders")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("", web::get().to(list_root))
        .route("", web::post().to(create_folder))
        .route("/{id}", web::get().to(list_folder))
        .route("/{id}/ancestors", web::get().to(get_ancestors))
        .route("/{id}/lock", web::post().to(set_lock))
        .route("/{id}/copy", web::post().to(copy_folder))
        .route("/{id}", web::delete().to(delete_folder))
        .route("/{id}", web::patch().to(patch_folder))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateFolderReq {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/folders",
    tag = "folders",
    operation_id = "create_folder",
    request_body = CreateFolderReq,
    responses(
        (status = 201, description = "Folder created", body = inline(ApiResponse<crate::entities::folder::Model>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    body: web::Json<CreateFolderReq>,
) -> Result<HttpResponse> {
    let folder = folder_service::create(&state, claims.user_id, &body.name, body.parent_id).await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::FolderCreate,
        Some("folder"),
        Some(folder.id),
        Some(&folder.name),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}

#[utoipa::path(
    get,
    path = "/api/v1/folders",
    tag = "folders",
    operation_id = "list_root",
    params(FolderListQuery),
    responses(
        (status = 200, description = "Root folder contents", body = inline(ApiResponse<folder_service::FolderContents>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_root(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<FolderListQuery>,
) -> Result<HttpResponse> {
    let contents = folder_service::list(
        &state,
        claims.user_id,
        None,
        query.folder_limit(),
        query.folder_offset(),
        query.file_limit(),
        query.file_cursor(),
        query.sort_by(),
        query.sort_order(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[utoipa::path(
    get,
    path = "/api/v1/folders/{id}",
    tag = "folders",
    operation_id = "list_folder",
    params(("id" = i64, Path, description = "Folder ID"), FolderListQuery),
    responses(
        (status = 200, description = "Folder contents", body = inline(ApiResponse<folder_service::FolderContents>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<FolderListQuery>,
) -> Result<HttpResponse> {
    let contents = folder_service::list(
        &state,
        claims.user_id,
        Some(*path),
        query.folder_limit(),
        query.folder_offset(),
        query.file_limit(),
        query.file_cursor(),
        query.sort_by(),
        query.sort_order(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[utoipa::path(
    get,
    path = "/api/v1/folders/{id}/ancestors",
    tag = "folders",
    operation_id = "get_folder_ancestors",
    params(("id" = i64, Path, description = "Folder ID")),
    responses(
        (status = 200, description = "Folder ancestors", body = inline(ApiResponse<Vec<folder_service::FolderAncestorItem>>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_ancestors(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let ancestors = folder_service::get_ancestors(&state, claims.user_id, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(ancestors)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/folders/{id}",
    tag = "folders",
    operation_id = "delete_folder",
    params(("id" = i64, Path, description = "Folder ID")),
    responses(
        (status = 200, description = "Folder deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let folder_id = *path;
    folder_service::delete(&state, folder_id, claims.user_id).await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::FolderDelete,
        Some("folder"),
        Some(folder_id),
        None,
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[derive(Deserialize, ToSchema)]
pub struct PatchFolderReq {
    pub name: Option<String>,
    #[serde(default)]
    #[schema(value_type = Option<i64>)]
    pub parent_id: NullablePatch<i64>,
    #[serde(default)]
    #[schema(value_type = Option<i64>)]
    pub policy_id: NullablePatch<i64>,
}

#[utoipa::path(
    patch,
    path = "/api/v1/folders/{id}",
    tag = "folders",
    operation_id = "patch_folder",
    params(("id" = i64, Path, description = "Folder ID")),
    request_body = PatchFolderReq,
    responses(
        (status = 200, description = "Folder updated", body = inline(ApiResponse<crate::entities::folder::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<PatchFolderReq>,
) -> Result<HttpResponse> {
    let folder = folder_service::update(
        &state,
        *path,
        claims.user_id,
        body.name.clone(),
        body.parent_id,
        body.policy_id,
    )
    .await?;
    let ctx = AuditContext::from_request(&req, &claims);
    let action = if body.parent_id.is_present() {
        audit_service::AuditAction::FolderMove
    } else {
        audit_service::AuditAction::FolderRename
    };
    audit_service::log(
        &state,
        &ctx,
        action,
        Some("folder"),
        Some(folder.id),
        Some(&folder.name),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(folder)))
}

// ── Lock ────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct SetLockReq {
    pub locked: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/folders/{id}/lock",
    tag = "folders",
    operation_id = "set_folder_lock",
    params(("id" = i64, Path, description = "Folder ID")),
    request_body = SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::entities::folder::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<SetLockReq>,
) -> Result<HttpResponse> {
    let folder = folder_service::set_lock(&state, *path, claims.user_id, body.locked).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(folder)))
}

// ── Copy ───────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CopyFolderReq {
    /// 目标父文件夹 ID（null = 根目录）
    pub parent_id: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/folders/{id}/copy",
    tag = "folders",
    operation_id = "copy_folder",
    params(("id" = i64, Path, description = "Source folder ID")),
    request_body = CopyFolderReq,
    responses(
        (status = 201, description = "Folder copied", body = inline(ApiResponse<crate::entities::folder::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn copy_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<CopyFolderReq>,
) -> Result<HttpResponse> {
    let folder = folder_service::copy_folder(&state, *path, claims.user_id, body.parent_id).await?;
    let ctx = AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::FolderCopy,
        Some("folder"),
        Some(folder.id),
        Some(&folder.name),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}
