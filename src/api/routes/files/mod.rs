use crate::api::middleware::auth::JwtAuth;
use crate::api::middleware::rate_limit;
use crate::api::response::ApiResponse;
use crate::api::routes::team_scope;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{auth_service::Claims, upload_service, version_service};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpRequest, HttpResponse, web};

pub(crate) mod access;
pub(crate) mod mutations;
pub(crate) mod upload;
pub(crate) mod versions;

pub use self::access::{
    OpenWopiRequest, download, get_direct_link, get_file, get_preview_link, get_thumbnail,
    open_wopi,
};
pub use self::mutations::{
    CopyFileReq, CreateEmptyRequest, ExtractArchiveRequest, PatchFileReq, SetLockReq, copy_file,
    create_empty, delete_file, extract_archive, patch_file, set_lock, update_content,
};
pub use self::upload::{
    ChunkPath, CompleteUploadReq, CompletedPartReq, FileQuery, InitUploadReq, PresignPartsReq,
    UploadIdPath, cancel_upload, complete_upload, get_upload_progress, init_chunked_upload,
    presign_parts, upload, upload_chunk,
};
pub use self::versions::{VersionPath, delete_version, list_versions, restore_version};

pub(crate) use self::access::{
    direct_link_response, download_response, get_file_response, get_thumbnail_response,
    open_wopi_response, preview_link_response, thumbnail_response,
};
pub(crate) use self::mutations::{
    copy_file_response, create_empty_response, delete_file_response, extract_archive_response,
    patch_file_response, set_lock_response, update_content_response,
};
pub(crate) use self::upload::upload_response;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.api);

    web::scope("/files")
        .wrap(JwtAuth)
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/upload", web::post().to(upload))
        .route("/new", web::post().to(create_empty))
        // chunked upload routes (before /{id} to avoid conflicts)
        .route("/upload/init", web::post().to(init_chunked_upload))
        .route(
            "/upload/{upload_id}/{chunk_number}",
            web::put().to(upload_chunk),
        )
        .route(
            "/upload/{upload_id}/complete",
            web::post().to(complete_upload),
        )
        .route(
            "/upload/{upload_id}/presign-parts",
            web::post().to(presign_parts),
        )
        .route("/upload/{upload_id}", web::get().to(get_upload_progress))
        .route("/upload/{upload_id}", web::delete().to(cancel_upload))
        .route("/{id}", web::get().to(get_file))
        .route("/{id}/direct-link", web::get().to(get_direct_link))
        .route("/{id}/preview-link", web::post().to(get_preview_link))
        .route("/{id}/wopi/open", web::post().to(open_wopi))
        .route("/{id}/download", web::get().to(download))
        .route("/{id}/thumbnail", web::get().to(get_thumbnail))
        .route("/{id}/content", web::put().to(update_content))
        .route("/{id}/extract", web::post().to(extract_archive))
        .route("/{id}/lock", web::post().to(set_lock))
        .route("/{id}/copy", web::post().to(copy_file))
        .route("/{id}/versions", web::get().to(list_versions))
        .route(
            "/{id}/versions/{version_id}/restore",
            web::post().to(restore_version),
        )
        .route(
            "/{id}/versions/{version_id}",
            web::delete().to(delete_version),
        )
        .route("/{id}", web::delete().to(delete_file))
        .route("/{id}", web::patch().to(patch_file))
}

pub fn team_routes() -> actix_web::Scope {
    web::scope("/files")
        .route("/upload", web::post().to(team_upload))
        .route("/upload/init", web::post().to(team_init_chunked_upload))
        .route(
            "/upload/{upload_id}/{chunk_number}",
            web::put().to(team_upload_chunk),
        )
        .route(
            "/upload/{upload_id}/complete",
            web::post().to(team_complete_upload),
        )
        .route(
            "/upload/{upload_id}/presign-parts",
            web::post().to(team_presign_parts),
        )
        .route(
            "/upload/{upload_id}",
            web::get().to(team_get_upload_progress),
        )
        .route("/upload/{upload_id}", web::delete().to(team_cancel_upload))
        .route("/new", web::post().to(team_create_empty))
        .route("/{id}", web::get().to(team_get_file))
        .route("/{id}/direct-link", web::get().to(team_get_direct_link))
        .route("/{id}/preview-link", web::post().to(team_get_preview_link))
        .route("/{id}/wopi/open", web::post().to(team_open_wopi))
        .route("/{id}/thumbnail", web::get().to(team_get_thumbnail))
        .route("/{id}/content", web::put().to(team_update_content))
        .route("/{id}/extract", web::post().to(team_extract_archive))
        .route("/{id}/lock", web::post().to(team_set_lock))
        .route("/{id}", web::patch().to(team_patch_file))
        .route("/{id}", web::delete().to(team_delete_file))
        .route("/{id}/copy", web::post().to(team_copy_file))
        .route("/{id}/versions", web::get().to(team_list_versions))
        .route(
            "/{id}/versions/{version_id}/restore",
            web::post().to(team_restore_version),
        )
        .route(
            "/{id}/versions/{version_id}",
            web::delete().to(team_delete_version),
        )
        .route("/{id}/download", web::get().to(team_download))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/teams/{team_id}/files/upload",
    tag = "teams",
    operation_id = "upload_team_file",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        FileQuery
    ),
    request_body(content = String, content_type = "multipart/form-data", description = "File to upload"),
    responses(
        (status = 201, description = "Team file uploaded", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<FileQuery>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    upload_response(
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
    request_body = InitUploadReq,
    responses(
        (status = 201, description = "Team upload session created", body = inline(ApiResponse<upload_service::InitUploadResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_init_chunked_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<InitUploadReq>,
) -> Result<HttpResponse> {
    let resp = upload_service::init_upload_for_team(
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
        (status = 200, description = "Chunk uploaded", body = inline(ApiResponse<upload_service::ChunkUploadResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_upload_chunk(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String, i32)>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (team_id, upload_id, chunk_number) = path.into_inner();
    let resp = upload_service::upload_chunk_for_team(
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
    request_body(content = CompleteUploadReq, description = "Multipart completion data (optional, only for presigned_multipart mode)", content_type = "application/json"),
    responses(
        (status = 201, description = "Team file created", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_complete_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
    body: Option<web::Json<CompleteUploadReq>>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let parts = body
        .and_then(|payload| payload.into_inner().parts)
        .map(|parts| {
            parts
                .into_iter()
                .map(|part| (part.part_number, part.etag))
                .collect()
        });
    let file = upload_service::complete_upload_for_team(
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
        (status = 200, description = "Upload progress", body = inline(ApiResponse<upload_service::UploadProgressResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_upload_progress(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let resp =
        upload_service::get_progress_for_team(&state, team_id, &upload_id, claims.user_id).await?;
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
pub(crate) async fn team_cancel_upload(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    upload_service::cancel_upload_for_team(&state, team_id, &upload_id, claims.user_id).await?;
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
    request_body = PresignPartsReq,
    responses(
        (status = 200, description = "Presigned URLs", body = inline(ApiResponse<std::collections::HashMap<i32, String>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Session not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_presign_parts(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, String)>,
    body: web::Json<PresignPartsReq>,
) -> Result<HttpResponse> {
    let (team_id, upload_id) = path.into_inner();
    let urls = upload_service::presign_parts_for_team(
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
    request_body = CreateEmptyRequest,
    responses(
        (status = 201, description = "Empty team file created", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_create_empty(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<CreateEmptyRequest>,
) -> Result<HttpResponse> {
    create_empty_response(&state, team_scope(*path, claims.user_id), &body).await
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
        (status = 200, description = "Team file info", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_get_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    get_file_response(&state, team_scope(team_id, claims.user_id), file_id).await
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
pub(crate) async fn team_get_direct_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    direct_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
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
pub(crate) async fn team_get_preview_link(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    preview_link_response(&state, team_scope(team_id, claims.user_id), file_id).await
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
    request_body = OpenWopiRequest,
    responses(
        (status = 200, description = "Team WOPI launch session", body = inline(ApiResponse<crate::services::wopi_service::WopiLaunchSession>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_open_wopi(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<OpenWopiRequest>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    open_wopi_response(
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
pub(crate) async fn team_get_thumbnail(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    get_thumbnail_response(&state, &req, team_scope(team_id, claims.user_id), file_id).await
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
        (status = 200, description = "Content updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
        (status = 412, description = "Precondition failed (ETag mismatch)"),
        (status = 423, description = "File is locked by another user"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_update_content(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    update_content_response(
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
    path = "/api/v1/teams/{team_id}/files/{id}/extract",
    tag = "teams",
    operation_id = "extract_team_file_archive",
    params(
        ("team_id" = i64, Path, description = "Team ID"),
        ("id" = i64, Path, description = "File ID")
    ),
    request_body = ExtractArchiveRequest,
    responses(
        (status = 200, description = "Team archive extract task created", body = inline(ApiResponse<crate::services::task_service::TaskInfo>)),
        (status = 400, description = "Unsupported archive format"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_extract_archive(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<ExtractArchiveRequest>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    extract_archive_response(&state, team_scope(team_id, claims.user_id), file_id, &body).await
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
    request_body = SetLockReq,
    responses(
        (status = 200, description = "Lock state updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_set_lock(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
    body: web::Json<SetLockReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    set_lock_response(
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
    request_body = PatchFileReq,
    responses(
        (status = 200, description = "Team file updated", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_patch_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<PatchFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    patch_file_response(
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
    request_body = CopyFileReq,
    responses(
        (status = 201, description = "Team file copied", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "File not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_copy_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
    body: web::Json<CopyFileReq>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    copy_file_response(
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
        (status = 200, description = "File versions", body = inline(ApiResponse<Vec<crate::services::workspace_models::FileVersion>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_list_versions(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    let versions =
        version_service::list_versions_for_team(&state, team_id, file_id, claims.user_id).await?;
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
        (status = 200, description = "Version restored", body = inline(ApiResponse<crate::services::workspace_models::FileInfo>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Version not found"),
    ),
    security(("bearer" = [])),
)]
pub(crate) async fn team_restore_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    let file = version_service::restore_version_for_team(
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
pub(crate) async fn team_delete_version(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id, version_id) = path.into_inner();
    version_service::delete_version_for_team(&state, team_id, file_id, version_id, claims.user_id)
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
pub(crate) async fn team_download(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    download_response(
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
pub(crate) async fn team_delete_file(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse> {
    let (team_id, file_id) = path.into_inner();
    delete_file_response(
        &state,
        &claims,
        &req,
        team_scope(team_id, claims.user_id),
        file_id,
    )
    .await
}
