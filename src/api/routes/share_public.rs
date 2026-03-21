use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::share_service;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use utoipa::ToSchema;

pub fn routes() -> actix_web::Scope {
    web::scope("/s")
        .route("/{token}", web::get().to(get_share_info))
        .route("/{token}/verify", web::post().to(verify_password))
        .route("/{token}/download", web::get().to(download_shared))
        .route("/{token}/content", web::get().to(list_shared_content))
        .route("/{token}/thumbnail", web::get().to(shared_thumbnail))
}

#[utoipa::path(
    get,
    path = "/api/v1/s/{token}",
    tag = "shares",
    operation_id = "get_share_info",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Share info", body = inline(ApiResponse<share_service::SharePublicInfo>)),
        (status = 404, description = "Share not found"),
        (status = 410, description = "Share expired"),
    ),
)]
pub async fn get_share_info(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let info = share_service::get_share_info(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(info)))
}

#[derive(Deserialize, ToSchema)]
pub struct VerifyPasswordReq {
    pub password: String,
}

#[utoipa::path(
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
    share_service::verify_password(&state, &path, &body.password).await?;

    // 设置一个短期 cookie 标记密码已验证
    let cookie = actix_web::cookie::Cookie::build(format!("aster_share_{}", &*path), "1")
        .path("/")
        .http_only(true)
        .max_age(actix_web::cookie::time::Duration::hours(1))
        .same_site(actix_web::cookie::SameSite::Lax)
        .finish();

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .json(ApiResponse::<()>::ok_empty()))
}

#[utoipa::path(
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
    // 检查密码验证 cookie（如果分享有密码）
    check_share_password_cookie(&state, &path, &req).await?;

    share_service::download_shared_file(&state, &path).await
}

#[utoipa::path(
    get,
    path = "/api/v1/s/{token}/content",
    tag = "shares",
    operation_id = "list_shared_content",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Folder contents", body = inline(ApiResponse<crate::api::response::FolderContentsResponse>)),
        (status = 403, description = "Password required"),
        (status = 404, description = "Share not found"),
    ),
)]
pub async fn list_shared_content(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    check_share_password_cookie(&state, &path, &req).await?;

    let contents = share_service::list_shared_folder(&state, &path).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(contents)))
}

#[utoipa::path(
    get,
    path = "/api/v1/s/{token}/thumbnail",
    tag = "shares",
    operation_id = "shared_thumbnail",
    params(("token" = String, Path, description = "Share token")),
    responses(
        (status = 200, description = "Thumbnail image (WebP)"),
        (status = 403, description = "Password required"),
        (status = 404, description = "Not found or not an image"),
    ),
)]
pub async fn shared_thumbnail(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> Result<HttpResponse> {
    check_share_password_cookie(&state, &path, &req).await?;

    use crate::db::repository::{file_repo, share_repo};
    use crate::errors::AsterError;
    use crate::services::thumbnail_service;

    let share = share_repo::find_by_token(&state.db, &path)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={}", &*path)))?;

    let file_id = share
        .file_id
        .ok_or_else(|| AsterError::validation_error("share is not a file"))?;

    let f = file_repo::find_by_id(&state.db, file_id).await?;
    if !thumbnail_service::is_supported_mime(&f.mime_type) {
        return Err(AsterError::thumbnail_generation_failed(
            "unsupported image type",
        ));
    }

    let blob = file_repo::find_blob_by_id(&state.db, f.blob_id).await?;
    let data = thumbnail_service::get_or_generate(&state, &blob).await?;

    Ok(HttpResponse::Ok()
        .content_type("image/webp")
        .insert_header(("Cache-Control", "public, max-age=31536000, immutable"))
        .body(data))
}

/// 如果分享有密码，检查是否已通过 cookie 验证
async fn check_share_password_cookie(
    state: &AppState,
    token: &str,
    req: &actix_web::HttpRequest,
) -> Result<()> {
    use crate::db::repository::share_repo;
    use crate::errors::AsterError;

    let share = share_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AsterError::share_not_found(format!("token={token}")))?;

    if share.password.is_some() {
        let cookie_name = format!("aster_share_{token}");
        if req.cookie(&cookie_name).is_none() {
            return Err(AsterError::share_password_required(
                "password verification required",
            ));
        }
    }
    Ok(())
}
