use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::api::routes::{files, folders};
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, workspace_storage_service::WorkspaceStorageScope};
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
        .route("/files/upload", web::post().to(upload))
        .route("/files/upload/init", web::post().to(init_chunked_upload))
        .route(
            "/files/upload/{upload_id}/{chunk_number}",
            web::put().to(upload_chunk),
        )
        .route(
            "/files/upload/{upload_id}/complete",
            web::post().to(complete_upload),
        )
        .route(
            "/files/upload/{upload_id}/presign-parts",
            web::post().to(presign_parts),
        )
        .route(
            "/files/upload/{upload_id}",
            web::get().to(get_upload_progress),
        )
        .route("/files/upload/{upload_id}", web::delete().to(cancel_upload))
        .route("/files/new", web::post().to(create_empty))
        .route("/files/{id}", web::get().to(get_file))
        .route("/files/{id}/direct-link", web::get().to(get_direct_link))
        .route("/files/{id}/preview-link", web::post().to(get_preview_link))
        .route("/files/{id}/wopi/open", web::post().to(open_wopi))
        .route("/files/{id}/thumbnail", web::get().to(get_thumbnail))
        .route("/files/{id}/content", web::put().to(update_content))
        .route("/files/{id}/lock", web::post().to(set_file_lock))
        .route("/files/{id}", web::patch().to(patch_file))
        .route("/files/{id}", web::delete().to(delete_file))
        .route("/files/{id}/copy", web::post().to(copy_file))
        .route("/files/{id}/versions", web::get().to(list_versions))
        .route(
            "/files/{id}/versions/{version_id}/restore",
            web::post().to(restore_version),
        )
        .route(
            "/files/{id}/versions/{version_id}",
            web::delete().to(delete_version),
        )
        .route("/files/{id}/download", web::get().to(download))
}

fn team_scope(team_id: i64, user_id: i64) -> WorkspaceStorageScope {
    WorkspaceStorageScope::Team {
        team_id,
        actor_user_id: user_id,
    }
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
        (status = 200, description = "Team root folder contents", body = inline(ApiResponse<crate::services::folder_service::FolderContents>)),
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
    folders::list_folder_response(&state, team_scope(*path, claims.user_id), None, &query).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/folders",
    tag = "teams",
    operation_id = "create_team_folder",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::folders::CreateFolderReq,
    responses(
        (status = 201, description = "Team folder created", body = inline(ApiResponse<crate::entities::folder::Model>)),
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
    folders::create_folder_response(
        &state,
        &claims,
        &req,
        team_scope(*path, claims.user_id),
        &body,
    )
    .await
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
        (status = 200, description = "Team folder contents", body = inline(ApiResponse<crate::services::folder_service::FolderContents>)),
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
    folders::list_folder_response(
        &state,
        team_scope(team_id, claims.user_id),
        Some(folder_id),
        &query,
    )
    .await
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
        (status = 200, description = "Team folder info", body = inline(ApiResponse<crate::entities::folder::Model>)),
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
    folders::get_folder_info_response(&state, team_scope(team_id, claims.user_id), folder_id).await
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
        (status = 200, description = "Team folder ancestors", body = inline(ApiResponse<Vec<crate::services::folder_service::FolderAncestorItem>>)),
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
    folders::get_ancestors_response(&state, team_scope(team_id, claims.user_id), folder_id).await
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
    folders::delete_folder_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        folder_id,
    )
    .await
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
        (status = 200, description = "Team folder updated", body = inline(ApiResponse<crate::entities::folder::Model>)),
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
    folders::patch_folder_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        folder_id,
        &body,
    )
    .await
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
        (status = 201, description = "Team folder copied", body = inline(ApiResponse<crate::entities::folder::Model>)),
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
    folders::copy_folder_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        folder_id,
        &body,
    )
    .await
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
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::entities::folder::Model>)),
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
    folders::set_lock_response(
        &state,
        team_scope(team_id, claims.user_id),
        folder_id,
        body.locked,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/upload",
    tag = "teams",
    operation_id = "upload_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        crate::api::routes::files::FileQuery
    ),
    request_body(content = String, content_type = "multipart/form-data", description = "File to upload"),
    responses(
        (status = 201, description = "Team file uploaded", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<crate::api::routes::files::FileQuery>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    files::upload_response(
        &state,
        &claims,
        &req,
        team_scope(*path, claims.user_id),
        query.folder_id,
        query.relative_path.as_deref(),
        query.declared_size,
        &mut payload,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/upload/init",
    tag = "teams",
    operation_id = "init_team_chunked_upload",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::files::InitUploadReq,
    responses(
        (status = 201, description = "Team upload session created", body = inline(ApiResponse<crate::services::upload_service::InitUploadResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn init_chunked_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::files::InitUploadReq>,
) -> Result<HttpResponse> {
    let resp = crate::services::upload_service::init_upload_for_team(
        &state,
        *path,
        claims.user_id,
        &body.filename,
        body.total_size,
        body.folder_id,
        body.relative_path.as_deref(),
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(resp)))
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/teams/{team_id}/files/upload/{upload_id}/{chunk_number}",
    tag = "teams",
    operation_id = "upload_team_chunk",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("upload_id" = String, Path, description = "Upload session ID"),
        ("chunk_number" = i32, Path, description = "Chunk number (0-indexed)")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Chunk uploaded", body = inline(ApiResponse<crate::services::upload_service::ChunkUploadResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload_chunk(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String, i32)>,
    body: actix_web::web::Bytes,
) -> Result<HttpResponse> {
    let (team_id, upload_id, chunk_number) = path.into_inner();
    let resp = crate::services::upload_service::upload_chunk_for_team(
        &state,
        team_id,
        &upload_id,
        chunk_number,
        claims.user_id,
        &body,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/upload/{upload_id}/complete",
    tag = "teams",
    operation_id = "complete_team_chunked_upload",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("upload_id" = String, Path, description = "Upload session ID")
    ),
    request_body(content = crate::api::routes::files::CompleteUploadReq, description = "Multipart completion data (optional, only for presigned_multipart mode)", content_type = "application/json"),
    responses(
        (status = 201, description = "Team file created", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn complete_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
    body: Option<web::Json<crate::api::routes::files::CompleteUploadReq>>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let parts = body
        .and_then(|b| b.into_inner().parts)
        .map(|parts| parts.into_iter().map(|p| (p.part_number, p.etag)).collect());
    let file = crate::services::upload_service::complete_upload_for_team(
        &state,
        team_id,
        &upload_id,
        claims.user_id,
        parts,
    )
    .await?;
    Ok(HttpResponse::Created().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/upload/{upload_id}",
    tag = "teams",
    operation_id = "get_team_upload_progress",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("upload_id" = String, Path, description = "Upload session ID")
    ),
    responses(
        (status = 200, description = "Upload progress", body = inline(ApiResponse<crate::services::upload_service::UploadProgressResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_upload_progress(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let resp = crate::services::upload_service::get_progress_for_team(
        &state,
        team_id,
        &upload_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(resp)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/files/upload/{upload_id}",
    tag = "teams",
    operation_id = "cancel_team_upload",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("upload_id" = String, Path, description = "Upload session ID")
    ),
    responses(
        (status = 200, description = "Upload cancelled"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn cancel_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    crate::services::upload_service::cancel_upload_for_team(
        &state,
        team_id,
        &upload_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/upload/{upload_id}/presign-parts",
    tag = "teams",
    operation_id = "presign_team_upload_parts",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("upload_id" = String, Path, description = "Upload session ID")
    ),
    request_body = crate::api::routes::files::PresignPartsReq,
    responses(
        (status = 200, description = "Presigned URLs", body = inline(ApiResponse<std::collections::HashMap<i32, String>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn presign_parts(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
    body: web::Json<crate::api::routes::files::PresignPartsReq>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let urls = crate::services::upload_service::presign_parts_for_team(
        &state,
        team_id,
        &upload_id,
        claims.user_id,
        body.part_numbers.clone(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(urls)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/new",
    tag = "teams",
    operation_id = "create_empty_team_file",
    params(("team_id" = i64, Path, description = "Team ID")),
    request_body = crate::api::routes::files::CreateEmptyRequest,
    responses(
        (status = 201, description = "Empty team file created", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_empty(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<crate::api::routes::files::CreateEmptyRequest>,
) -> Result<HttpResponse> {
    files::create_empty_response(&state, team_scope(*path, claims.user_id), &body).await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "get_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file info", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::get_file_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/direct-link",
    tag = "teams",
    operation_id = "get_team_file_direct_link",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file direct link token", body = inline(ApiResponse<crate::services::direct_link_service::DirectLinkTokenInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_direct_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::direct_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/preview-link",
    tag = "teams",
    operation_id = "create_team_file_preview_link",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file preview link", body = inline(ApiResponse<crate::services::preview_link_service::PreviewLinkInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_preview_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::preview_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/wopi/open",
    tag = "teams",
    operation_id = "open_team_file_with_wopi",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = crate::api::routes::files::OpenWopiRequest,
    responses(
        (status = 200, description = "Team WOPI launch session", body = inline(ApiResponse<crate::services::wopi_service::WopiLaunchSession>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn open_wopi(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::files::OpenWopiRequest>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::open_wopi_response(
        &state,
        team_scope(team_id, claims.user_id),
        file_id,
        &body.app_key,
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/thumbnail",
    tag = "teams",
    operation_id = "get_team_thumbnail",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 304, description = "Thumbnail not modified"),
        (status = 202, description = "Thumbnail generation in progress"),
        (status = 400, description = "Thumbnail not supported for this file type"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
        (status = 500, description = "Thumbnail generation failed"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_thumbnail(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::get_thumbnail_response(&state, &req, team_scope(team_id, claims.user_id), file_id).await
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/teams/{team_id}/files/{id}/content",
    tag = "teams",
    operation_id = "update_team_file_content",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Content updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
        (status = 412, description = "Precondition failed (ETag mismatch)"),
        (status = 423, description = "File is locked by another user"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_content(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::update_content_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/lock",
    tag = "teams",
    operation_id = "set_team_file_lock",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = crate::api::routes::files::SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn set_file_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::files::SetLockReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::set_lock_response(
        &state,
        team_scope(team_id, claims.user_id),
        file_id,
        body.locked,
    )
    .await
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "patch_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = crate::api::routes::files::PatchFileReq,
    responses(
        (status = 200, description = "Team file updated", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::files::PatchFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::patch_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        &body,
    )
    .await
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/copy",
    tag = "teams",
    operation_id = "copy_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "Source file ID")
    ),
    request_body = crate::api::routes::files::CopyFileReq,
    responses(
        (status = 201, description = "Team file copied", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn copy_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<crate::api::routes::files::CopyFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::copy_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
        &body,
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/versions",
    tag = "teams",
    operation_id = "list_team_versions",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "File versions", body = inline(ApiResponse<Vec<crate::entities::file_version::Model>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_versions(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    let versions = crate::services::version_service::list_versions_for_team(
        &state,
        team_id,
        file_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(versions)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/{id}/versions/{version_id}/restore",
    tag = "teams",
    operation_id = "restore_team_version",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version restored", body = inline(ApiResponse<crate::entities::file::Model>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn restore_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    let file = crate::services::version_service::restore_version_for_team(
        &state,
        team_id,
        file_id,
        version_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(file)))
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/files/{id}/versions/{version_id}",
    tag = "teams",
    operation_id = "delete_team_version",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID"),
        ("version_id" = i64, Path, description = "Version ID"),
    ),
    responses(
        (status = 200, description = "Version deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    crate::services::version_service::delete_version_for_team(
        &state,
        team_id,
        file_id,
        version_id,
        claims.user_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/teams/{team_id}/files/{id}/download",
    tag = "teams",
    operation_id = "download_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file content"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::download_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
    )
    .await
}

#[api_docs_macros::path(
    delete,
    path = "/api/v1/teams/{team_id}/files/{id}",
    tag = "teams",
    operation_id = "delete_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    responses(
        (status = 200, description = "Team file deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    files::delete_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
    )
    .await
}
