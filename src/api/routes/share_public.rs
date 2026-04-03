use crate::api::constants::YEAR_SECS;
use crate::api::middleware::rate_limit;
use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{profile_service, share_service};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub fn routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.public);
    let verify_limiter = rate_limit::build_governor(&rl.auth);

    web::scope("/s")
        .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
        .route("/{token}", web::get().to(get_share_info))
        .service(
            web::resource("/{token}/verify")
                .wrap(Condition::new(rl.enabled, Governor::new(&verify_limiter)))
                .route(web::post().to(verify_password)),
        )
        .route("/{token}/download", web::get().to(download_shared))
        .route(
            "/{token}/files/{file_id}/download",
            web::get().to(download_shared_folder_file),
        )
        .route("/{token}/content", web::get().to(list_shared_content))
        .route(
            "/{token}/folders/{folder_id}/content",
            web::get().to(list_shared_subfolder_content),
        )
        .route("/{token}/thumbnail", web::get().to(shared_thumbnail))
        .route(
            "/{token}/files/{file_id}/thumbnail",
            web::get().to(shared_folder_file_thumbnail),
        )
        .route("/{token}/avatar/{size}", web::get().to(shared_avatar))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}",
    tag = "shares",
    operation_id = "get_share_info",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Share info", body = inline(ApiResponse<share_service::SharePublicInfo>)),
        (status = 404, description = "Share not found or expired"),
    ),
)]
pub async fn get_share_info(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let info = share_service::get_share_info(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(info)))
}

#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct VerifyPasswordReq {
    pub password: String,
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/s/{token}/verify",
    tag = "shares",
    operation_id = "verify_share_password",
    params(("token" = String, Path, description = "Share token")),
    request_body = VerifyPasswordReq,
    responses(
        (status = 200, description = "Password verified"),
        (status = 401, description = "Wrong password"),
        (status = 404, description = "Share not found"),
    ),
)]
pub async fn verify_password(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<VerifyPasswordReq>,
) -> Result<HttpResponse> {
    let result = share_service::verify_password_and_sign(&state, &path, &body.password).await?;

    let cookie = actix_web::cookie::Cookie::build(
        format!("aster_share_{}", &*path),
        result.cookie_signature,
    )
    .path("/")
    .http_only(true)
    .max_age(actix_web::cookie::time::Duration::hours(1))
    .same_site(actix_web::cookie::SameSite::Lax)
    .finish();

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}/download",
    tag = "shares",
    operation_id = "download_shared_file",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "File content"),
        (status = 403, description = "Password required or download limit"),
        (status = 404, description = "Share not found"),
    ),
)]
pub async fn download_shared(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let cookie_value = req
        .cookie(&format!("aster_share_{}", &*path))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &path, cookie_value.as_deref()).await?;

    share_service::download_shared_file(
        &state,
        &path,
        req.headers()
            .get("If-None-Match")
            .and_then(|v| v.to_str().ok()),
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}/files/{file_id}/download",
    tag = "shares",
    operation_id = "download_shared_folder_file",
    params(
        ("token" = String, Path, description = "Share token"),
        ("file_id" = i64, Path, description = "File ID inside shared folder")
    ),
    responses(
        (status = 200, description = "File content"),
        (status = 403, description = "Password required or file outside shared folder"),
        (status = 404, description = "Share or file not found"),
    )
)]
pub async fn download_shared_folder_file(
    state: web::Data<AppState>,
    path: web::Path<(String, i64)>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, file_id) = path.into_inner();
    let cookie_value = req
        .cookie(&format!("aster_share_{token}"))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    share_service::download_shared_folder_file(
        &state,
        &token,
        file_id,
        req.headers()
            .get("If-None-Match")
            .and_then(|v| v.to_str().ok()),
    )
    .await
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}/content",
    tag = "shares",
    operation_id = "list_shared_content",
    params(("token" = String, Path, description = "Share token"), FolderListQuery),
    responses(
        (status = 200, description = "Folder contents", body = inline(ApiResponse<crate::services::folder_service::FolderContents>)),
        (status = 403, description = "Password required"),
        (status = 404, description = "Share not found"),
    ),
)]
pub async fn list_shared_content(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<FolderListQuery>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let cookie_value = req
        .cookie(&format!("aster_share_{}", &*path))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &path, cookie_value.as_deref()).await?;

    let contents = share_service::list_shared_folder(
        &state,
        &path,
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
    path = "/api/v1/s/{token}/folders/{folder_id}/content",
    tag = "shares",
    operation_id = "list_shared_subfolder_content",
    params(
        ("token" = String, Path, description = "Share token"),
        ("folder_id" = i64, Path, description = "Subfolder ID inside shared folder"),
        FolderListQuery,
    ),
    responses(
        (status = 200, description = "Subfolder contents", body = inline(ApiResponse<crate::services::folder_service::FolderContents>)),
        (status = 403, description = "Password required or folder outside shared scope"),
        (status = 404, description = "Share or folder not found"),
    )
)]
pub async fn list_shared_subfolder_content(
    state: web::Data<AppState>,
    path: web::Path<(String, i64)>,
    query: web::Query<FolderListQuery>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, folder_id) = path.into_inner();
    let cookie_value = req
        .cookie(&format!("aster_share_{token}"))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    let contents = share_service::list_shared_subfolder(
        &state,
        &token,
        folder_id,
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
    path = "/api/v1/s/{token}/avatar/{size}",
    tag = "shares",
    operation_id = "shared_avatar",
    params(
        ("token" = String, Path, description = "Share token"),
        ("size" = u32, Path, description = "Avatar size (512 or 1024)"),
    ),
    responses(
        (status = 200, description = "Share owner avatar image (WebP)"),
        (status = 403, description = "Password required"),
        (status = 404, description = "Share or avatar not found"),
    )
)]
pub async fn shared_avatar(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, size) = path.into_inner();
    let cookie_value = req
        .cookie(&format!("aster_share_{token}"))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    let bytes = share_service::get_share_avatar_bytes(&state, &token, size).await?;
    Ok(profile_service::avatar_image_response(bytes))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}/thumbnail",
    tag = "shares",
    operation_id = "shared_thumbnail",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 400, description = "Thumbnail not supported for this file type"),
        (status = 403, description = "Password required"),
        (status = 404, description = "Share or file not found"),
        (status = 500, description = "Thumbnail generation failed"),
    ),
)]
pub async fn shared_thumbnail(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let cookie_value = req
        .cookie(&format!("aster_share_{}", &*path))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &path, cookie_value.as_deref()).await?;

    let data = share_service::get_shared_thumbnail(&state, &path).await?;

    Ok(HttpResponse::Ok()
        .content_type("image/webp")
        .insert_header((
            "Cache-Control",
            format!("public, max-age={YEAR_SECS}, immutable"),
        ))
        .body(data))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/s/{token}/files/{file_id}/thumbnail",
    tag = "shares",
    operation_id = "shared_folder_file_thumbnail",
    params(
        ("token" = String, Path, description = "Share token"),
        ("file_id" = i64, Path, description = "File ID inside shared folder")
    ),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 400, description = "Thumbnail not supported for this file type"),
        (status = 403, description = "Password required or file outside shared scope"),
        (status = 404, description = "Share or file not found"),
        (status = 500, description = "Thumbnail generation failed"),
    )
)]
pub async fn shared_folder_file_thumbnail(
    state: web::Data<AppState>,
    path: web::Path<(String, i64)>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, file_id) = path.into_inner();
    let cookie_value = req
        .cookie(&format!("aster_share_{token}"))
        .map(|c| c.value().to_string());
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    let data = share_service::get_shared_folder_file_thumbnail(&state, &token, file_id).await?;

    Ok(HttpResponse::Ok()
        .content_type("image/webp")
        .insert_header((
            "Cache-Control",
            format!("public, max-age={YEAR_SECS}, immutable"),
        ))
        .body(data))
}
