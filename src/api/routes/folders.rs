use crate::api::middleware::auth::JwtAuth;
use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::{self, AuditContext},
    auth_service::Claims,
    folder_service,
};
use crate::types::EntityType;
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/folders")
        .wrap(JwtAuth)
        .route("", web::get().to(list_root))
        .route("", web::post().to(create_folder))
        .route("/{id}", web::get().to(list_folder))
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
        "folder_create",
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
        query.file_offset(),
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
        query.file_offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
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
        "folder_delete",
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
    pub parent_id: Option<i64>,
    pub policy_id: Option<i64>,
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
    let action = if body.parent_id.is_some() {
        "folder_move"
    } else {
        "folder_rename"
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
    use crate::services::lock_service;
    if body.locked {
        lock_service::lock(
            &state,
            EntityType::Folder,
            *path,
            Some(claims.user_id),
            None,
            None,
        )
        .await?;
    } else {
        lock_service::unlock(&state, EntityType::Folder, *path, claims.user_id).await?;
    }
    // 返回更新后的文件夹信息
    let folder = crate::db::repository::folder_repo::find_by_id(&state.db, *path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(folder)))
}

// ── Copy ───────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CopyFolderReq {
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
        "folder_copy",
        Some("folder"),
        Some(folder.id),
        Some(&folder.name),
        None,
    )
    .await;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}
