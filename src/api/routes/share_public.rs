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

    // 设置签名 cookie 标记密码已验证
    let signature = sign_share_cookie(&path, &state.config.auth.jwt_secret);
    let cookie = actix_web::cookie::Cookie::build(format!("aster_share_{}", &*path), signature)
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

/// SHA256 签名：防止伪造分享密码验证 cookie
fn sign_share_cookie(token: &str, secret: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(format!("share_verified:{secret}:{token}").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 验证分享密码 cookie 签名（常量时间比较）
fn verify_share_cookie(token: &str, cookie_value: &str, secret: &str) -> bool {
    let expected = sign_share_cookie(token, secret);
    // 长度不同直接 false，避免泄漏长度信息
    if expected.len() != cookie_value.len() {
        return false;
    }
    // 常量时间比较
    expected
        .bytes()
        .zip(cookie_value.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// 如果分享有密码，检查 cookie 签名是否有效
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
        let cookie = req
            .cookie(&cookie_name)
            .ok_or_else(|| AsterError::share_password_required("password verification required"))?;

        if !verify_share_cookie(token, cookie.value(), &state.config.auth.jwt_secret) {
            return Err(AsterError::share_password_required(
                "invalid verification cookie",
            ));
        }
    }
    Ok(())
}
