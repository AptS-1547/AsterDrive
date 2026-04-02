use crate::api::pagination::TrashListQuery;
use crate::api::response::{ApiResponse, PurgedCountResponse};
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, trash_service};
use crate::types::EntityType;
use actix_web::{HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/{team_id}/trash")
        .route("", web::get().to(list_trash))
        .route("", web::delete().to(purge_all))
        .route("/{entity_type}/{id}/restore", web::post().to(restore))
        .route("/{entity_type}/{id}", web::delete().to(purge_one))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/trash",
    tag = "teams",
    operation_id = "list_team_trash",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        TrashListQuery
    ),
    responses(
        (status = 200, description = "Team trash contents", body = inline(ApiResponse<trash_service::TrashContents>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_trash(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<TrashListQuery>,
) -> Result<HttpResponse> {
    let team_id = *path;
    let contents = trash_service::list_team_trash(
        &state,
        team_id,
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
    path = "/api/v1/teams/{team_id}/trash/{entity_type}/{id}/restore",
    tag = "teams",
    operation_id = "restore_team_trash_item",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("entity_type" = EntityType, Path, description = "file or folder"),
        ("id" = i64, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "Restored"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn restore(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, EntityType, i64)>,
) -> Result<HttpResponse> {
    let (team_id, entity_type, id) = path.into_inner();
    match entity_type {
        EntityType::File => {
            trash_service::restore_team_file(&state, team_id, id, claims.user_id).await?
        }
        EntityType::Folder => {
            trash_service::restore_team_folder(&state, team_id, id, claims.user_id).await?
        }
    }
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/trash/{entity_type}/{id}",
    tag = "teams",
    operation_id = "purge_team_trash_item",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("entity_type" = EntityType, Path, description = "file or folder"),
        ("id" = i64, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "Permanently deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn purge_one(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, EntityType, i64)>,
) -> Result<HttpResponse> {
    let (team_id, entity_type, id) = path.into_inner();
    match entity_type {
        EntityType::File => {
            trash_service::purge_team_file(&state, team_id, id, claims.user_id).await?
        }
        EntityType::Folder => {
            trash_service::purge_team_folder(&state, team_id, id, claims.user_id).await?
        }
    }
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/trash",
    tag = "teams",
    operation_id = "purge_all_team_trash",
    params(("team_id" = i64, Path, description = "Team ID")),
    responses(
        (status = 200, description = "Trash emptied", body = inline(ApiResponse<PurgedCountResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn purge_all(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let purged = trash_service::purge_all_team(&state, *path, claims.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(PurgedCountResponse { purged })))
}
