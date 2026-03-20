use crate::api::middleware::auth::JwtAuth;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, folder_service};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/folders")
        .wrap(JwtAuth)
        .route("", web::get().to(list_root))
        .route("", web::post().to(create_folder))
        .route("/{id}", web::get().to(list_folder))
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
    body: web::Json<CreateFolderReq>,
) -> Result<HttpResponse> {
    let folder =
        folder_service::create(&state.db, claims.user_id, &body.name, body.parent_id).await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}

#[utoipa::path(
    get,
    path = "/api/v1/folders",
    tag = "folders",
    operation_id = "list_root",
    responses(
        (status = 200, description = "Root folder contents", body = inline(ApiResponse<crate::api::response::FolderContentsResponse>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_root(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let contents = folder_service::list(&state.db, claims.user_id, None).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[utoipa::path(
    get,
    path = "/api/v1/folders/{id}",
    tag = "folders",
    operation_id = "list_folder",
    params(("id" = i64, Path, description = "Folder ID")),
    responses(
        (status = 200, description = "Folder contents", body = inline(ApiResponse<crate::api::response::FolderContentsResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let contents = folder_service::list(&state.db, claims.user_id, Some(*path)).await?;
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
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    folder_service::delete(&state.db, *path, claims.user_id).await?;
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
    path: web::Path<i64>,
    body: web::Json<PatchFolderReq>,
) -> Result<HttpResponse> {
    let folder = folder_service::update(
        &state.db,
        *path,
        claims.user_id,
        body.name.clone(),
        body.parent_id,
        body.policy_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(folder)))
}
