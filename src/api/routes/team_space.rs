use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::api::routes::{files, team_scope};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    audit_service::AuditContext, auth_service::Claims, folder_service, workspace_models::FolderInfo,
};
use actix_web::{HttpRequest, HttpResponse, web};

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let _ = rl;

    web::scope("/{team_id}")
        .route("/folders", web::get().to(list_root))
        .route("/folders", web::post().to(create_folder))
        .route("/folders/{id}", web::get().to(list_folder))
        .route("/folders/{id}/info", web::get().to(get_folder_info))
        .route("/folders/{id}", web::patch().to(patch_folder))
        .route("/folders/{id}", web::delete().to(delete_folder))
        .route("/folders/{id}/lock", web::post().to(set_folder_lock))
        .route("/folders/{id}/copy", web::post().to(copy_folder))
        .route("/folders/{id}/ancestors", web::get().to(get_ancestors))
        .service(files::team_routes())
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/folders",
    tag = "teams",
    operation_id = "list_team_root",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        FolderListQuery
    ),
    responses(
        (status = 200, description = "Team root folder contents", body = inline(crate::api::response::ApiResponse<crate::services::folder_service::FolderContents>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_root(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    query: web::Query<FolderListQuery>,
) -> Result<HttpResponse> {
    let contents = folder_service::list_in_scope(
        &state,
        team_scope(*path, claims.user_id),
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

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/folders",
    tag = "teams",
    operation_id = "create_team_folder",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::folders::CreateFolderReq,
    responses(
        (status = 201, description = "Team folder created", body = inline(crate::api::response::ApiResponse<crate::services::workspace_models::FolderInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    req: HttpRequest,
    body: web::Json<crate::api::routes::folders::CreateFolderReq>,
) -> Result<HttpResponse> {
    let ctx = AuditContext::from_request(&req, &claims);
    let folder = folder_service::create_in_scope_with_audit(
        &state,
        team_scope(*path, claims.user_id),
        &body.name,
        body.parent_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/folders/{id}",
    tag = "teams",
    operation_id = "list_team_folder",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID"),
        FolderListQuery
    ),
    responses(
        (status = 200, description = "Team folder contents", body = inline(crate::api::response::ApiResponse<crate::services::folder_service::FolderContents>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    query: web::Query<FolderListQuery>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let contents = folder_service::list_in_scope(
        &state,
        team_scope(team_id, claims.user_id),
        Some(folder_id),
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

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/folders/{id}/info",
    tag = "teams",
    operation_id = "get_team_folder_info",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID")
    ),
    responses(
        (status = 200, description = "Team folder info", body = inline(crate::api::response::ApiResponse<crate::services::workspace_models::FolderInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_folder_info(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let folder =
        folder_service::get_info_in_scope(&state, team_scope(team_id, claims.user_id), folder_id)
            .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(FolderInfo::from(folder))))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/folders/{id}/ancestors",
    tag = "teams",
    operation_id = "get_team_folder_ancestors",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID")
    ),
    responses(
        (status = 200, description = "Team folder ancestors", body = inline(crate::api::response::ApiResponse<Vec<crate::services::folder_service::FolderAncestorItem>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_ancestors(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let ancestors = folder_service::get_ancestors_in_scope(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(ancestors)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/folders/{id}",
    tag = "teams",
    operation_id = "delete_team_folder",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID")
    ),
    responses(
        (status = 200, description = "Team folder deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    folder_service::delete_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{team_id}/folders/{id}",
    tag = "teams",
    operation_id = "patch_team_folder",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID")
    ),
    request_body = crate::api::routes::folders::PatchFolderReq,
    responses(
        (status = 200, description = "Team folder updated", body = inline(crate::api::response::ApiResponse<crate::services::workspace_models::FolderInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::folders::PatchFolderReq>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    let folder = folder_service::update_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
        body.name.clone(),
        body.parent_id,
        body.policy_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(folder)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/folders/{id}/copy",
    tag = "teams",
    operation_id = "copy_team_folder",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Source folder ID")
    ),
    request_body = crate::api::routes::folders::CopyFolderReq,
    responses(
        (status = 201, description = "Team folder copied", body = inline(crate::api::response::ApiResponse<crate::services::workspace_models::FolderInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn copy_folder(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::folders::CopyFolderReq>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let ctx = AuditContext::from_request(&req, &claims);
    let folder = folder_service::copy_folder_in_scope_with_audit(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
        body.parent_id,
        &ctx,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(folder)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/folders/{id}/lock",
    tag = "teams",
    operation_id = "set_team_folder_lock",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Folder ID")
    ),
    request_body = crate::api::routes::folders::SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(crate::api::response::ApiResponse<crate::services::workspace_models::FolderInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Folder not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_folder_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::folders::SetLockReq>,
) -> Result<HttpResponse> {
    let (team_id, folder_id) = path.into_inner();
    let folder = folder_service::set_lock_in_scope(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
        body.locked,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(FolderInfo::from(folder))))
}
