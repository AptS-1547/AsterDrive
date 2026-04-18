use crate::api::dto::TrashItemPath;
use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::pagination::TrashListQuery;
use crate::api::response::{ApiResponse, PurgedCountResponse};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, trash_service};
use crate::types::EntityType;
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/trash")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("", web::get().to(list_trash))
        .route("", web::delete().to(purge_all))
        .route("/{entity_type}/{id}/restore", web::post().to(restore))
        .route("/{entity_type}/{id}", web::delete().to(purge_one))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/trash",
    tag = "trash",
    operation_id = "list_trash",
    params(TrashListQuery),
    responses(
        (status = 200, description = "Trash contents", body = inline(ApiResponse<trash_service::TrashContents>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_trash(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<TrashListQuery>,
) -> Result<HttpResponse> {
    let contents = trash_service::list_trash(
        &state,
        claims.user_id,
        query.folder_limit(),
        query.folder_offset(),
        query.file_limit(),
        query.file_cursor(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[api_docs_macros::path(
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

#[api_docs_macros::path(
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

#[api_docs_macros::path(
    delete,
    path = "/api/v1/trash",
    tag = "trash",
    operation_id = "purge_all_trash",
    responses(
        (status = 200, description = "Trash emptied", body = inline(ApiResponse<crate::api::response::PurgedCountResponse>)),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
)]
pub async fn purge_all(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let count = trash_service::purge_all(&state, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(PurgedCountResponse { purged: count })))
}
