use crate::api::constants::YEAR_SECS;
use crate::api::middleware::rate_limit;
use crate::api::pagination::FolderListQuery;
use crate::api::response::ApiResponse;
use crate::config::RateLimitConfig;
use crate::config::auth_runtime::RuntimeAuthPolicy;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{direct_link_service, preview_link_service, profile_service, share_service};
use actix_governor::Governor;
use actix_web::middleware::Condition;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

const SHARE_COOKIE_PREFIX: &str = "aster_share_";

fn share_cookie_name(token: &str) -> String {
    format!("{SHARE_COOKIE_PREFIX}{token}")
}

fn share_cookie_path(token: &str) -> String {
    format!("/api/v1/s/{token}")
}

fn build_share_cookie(
    token: &str,
    value: String,
    secure: bool,
) -> actix_web::cookie::Cookie<'static> {
    actix_web::cookie::Cookie::build(share_cookie_name(token), value)
        .path(share_cookie_path(token))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(secure)
        .max_age(actix_web::cookie::time::Duration::hours(1))
        .finish()
}

fn share_cookie_value(req: &actix_web::HttpRequest, token: &str) -> Option<String> {
    req.cookie(&share_cookie_name(token))
        .map(|cookie| cookie.value().to_string())
}

#[derive(Deserialize, Default)]
pub struct DirectLinkQuery {
    pub download: Option<String>,
}

impl DirectLinkQuery {
    fn force_download(&self) -> bool {
        self.download
            .as_deref()
            .map(|value| matches!(value, "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
    }
}

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
        .route("/{token}/preview-link", web::post().to(create_preview_link))
        .route("/{token}/download", web::get().to(download_shared))
        .route(
            "/{token}/files/{file_id}/download",
            web::get().to(download_shared_folder_file),
        )
        .route(
            "/{token}/files/{file_id}/preview-link",
            web::post().to(create_folder_file_preview_link),
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

pub fn direct_routes(rl: &RateLimitConfig) -> impl actix_web::dev::HttpServiceFactory + use<> {
    let limiter = rate_limit::build_governor(&rl.public);

    (
        web::resource("/d/{token}/{filename}")
            .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
            .route(web::get().to(download_direct)),
        web::resource("/pv/{token}/{filename}")
            .wrap(Condition::new(rl.enabled, Governor::new(&limiter)))
            .route(web::get().to(download_preview)),
    )
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
    let auth_policy = RuntimeAuthPolicy::from_runtime_config(&state.runtime_config);
    let cookie = build_share_cookie(
        path.as_str(),
        result.cookie_signature,
        auth_policy.cookie_secure,
    );

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .json(ApiResponse::<()>::ok_empty()))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/s/{token}/preview-link",
    tag = "shares",
    operation_id = "create_shared_file_preview_link",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Preview link", body = inline(ApiResponse<crate::services::preview_link_service::PreviewLinkInfo>)),
        (status = 403, description = "Password required or download limit"),
        (status = 404, description = "Share not found"),
    ),
)]
pub async fn create_preview_link(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let token = path.into_inner();
    let cookie_value = share_cookie_value(&req, &token);
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    let link = preview_link_service::create_token_for_shared_file(&state, &token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(link)))
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
    let cookie_value = share_cookie_value(&req, path.as_str());
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

pub async fn download_direct(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    query: web::Query<DirectLinkQuery>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, filename) = path.into_inner();
    direct_link_service::download_file(
        &state,
        &token,
        &filename,
        query.force_download(),
        req.headers()
            .get("If-None-Match")
            .and_then(|v| v.to_str().ok()),
    )
    .await
}

pub async fn download_preview(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, filename) = path.into_inner();
    preview_link_service::download_file(
        &state,
        &token,
        &filename,
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
    let cookie_value = share_cookie_value(&req, &token);
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
    post,
    path = "/api/v1/s/{token}/files/{file_id}/preview-link",
    tag = "shares",
    operation_id = "create_shared_folder_file_preview_link",
    params(
        ("token" = String, Path, description = "Share token"),
        ("file_id" = i64, Path, description = "File ID inside shared folder")
    ),
    responses(
        (status = 200, description = "Preview link", body = inline(ApiResponse<crate::services::preview_link_service::PreviewLinkInfo>)),
        (status = 403, description = "Password required or file outside shared folder"),
        (status = 404, description = "Share or file not found"),
    )
)]
pub async fn create_folder_file_preview_link(
    state: web::Data<AppState>,
    path: web::Path<(String, i64)>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    let (token, file_id) = path.into_inner();
    let cookie_value = share_cookie_value(&req, &token);
    share_service::check_share_password_cookie(&state, &token, cookie_value.as_deref()).await?;

    let link =
        preview_link_service::create_token_for_shared_folder_file(&state, &token, file_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(link)))
}

#[cfg(test)]
mod tests {
    use super::direct_routes;
    use crate::config::RateLimitConfig;
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};

    #[actix_web::test]
    async fn direct_routes_do_not_shadow_later_root_services() {
        let app = test::init_service(
            App::new()
                .service(direct_routes(&RateLimitConfig::default()))
                .route(
                    "/after",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;

        let req = test::TestRequest::get().uri("/after").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }
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
    let cookie_value = share_cookie_value(&req, path.as_str());
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
    let cookie_value = share_cookie_value(&req, &token);
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
    let cookie_value = share_cookie_value(&req, &token);
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
    let cookie_value = share_cookie_value(&req, path.as_str());
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
    let cookie_value = share_cookie_value(&req, &token);
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
