use crate::api::middleware::auth::JwtAuth;
use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, trash_service};
use crate::types::EntityType;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> impl actix_web::dev::HttpServiceFactory {
    web::scope("/trash")
        .wrap(JwtAuth)
        .route("", web::get().to(list_trash))
        .route("", web::delete().to(purge_all))
        .route("/{entity_type}/{id}/restore", web::post().to(restore))
        .route("/{entity_type}/{id}", web::delete().to(purge_one))
}

#[derive(Deserialize, ToSchema)]
pub struct TrashItemPath {
    pub entity_type: EntityType,
    pub id: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/trash",
    tag = "trash",
    operation_id = "list_trash",
    params(FolderListQuery),
    responses(
        (status = 200, description = "Trash contents", body = inline(ApiResponse<trash_service::TrashContents>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_trash(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<FolderListQuery>,
) -> Result<HttpResponse> {
    let contents = trash_service::list_trash(
        &state,
        claims.user_id,
        query.folder_limit(),
        query.folder_offset(),
        query.file_limit(),
        query.file_offset(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[utoipa::path(
    post,
    path = "/api/v1/trash/{entity_type}/{id}/restore",
    tag = "trash",
    operation_id = "restore_from_trash",
    params(
        ("entity_type" = EntityType, Path, description = "file or folder"),
        ("id" = i64, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "Restored"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn restore(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<TrashItemPath>,
) -> Result<HttpResponse> {
    match path.entity_type {
        EntityType::File => trash_service::restore_file(&state, path.id, claims.user_id).await?,
        EntityType::Folder => {
            trash_service::restore_folder(&state, path.id, claims.user_id).await?
        }
    }
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/trash/{entity_type}/{id}",
    tag = "trash",
    operation_id = "purge_from_trash",
    params(
        ("entity_type" = EntityType, Path, description = "file or folder"),
        ("id" = i64, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "Permanently deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn purge_one(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<TrashItemPath>,
) -> Result<HttpResponse> {
    match path.entity_type {
        EntityType::File => trash_service::purge_file(&state, path.id, claims.user_id).await?,
        EntityType::Folder => trash_service::purge_folder(&state, path.id, claims.user_id).await?,
    }
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/trash",
    tag = "trash",
    operation_id = "purge_all_trash",
    responses(
        (status = 200, description = "Trash emptied"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn purge_all(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let count = trash_service::purge_all(&state, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "purged": count }))))
}
