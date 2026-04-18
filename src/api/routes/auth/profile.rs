//! 认证 API 路由：`profile`。

use super::{
    ActionMessageResp, RequestEmailChangeReq, UpdateAvatarSourceReq, UpdatePreferencesReq,
    UpdateProfileReq, apply_auth_mail_response_floor,
};
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;
use crate::services::{audit_service, auth_service, profile_service, user_service};
use actix_web::{HttpRequest, HttpResponse, web};

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/email/change",
    tag = "auth",
    operation_id = "request_email_change",
    request_body = RequestEmailChangeReq,
    responses(
        (status = 200, description = "Email change requested", body = inline(ApiResponse<crate::api::routes::auth::UserInfo>)),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Account pending activation"),
    ),
    security(("bearer" = [])),
)]
pub async fn request_email_change(
    state: web::Data<AppState>,
    req: HttpRequest,
    claims: web::ReqData<Claims>,
    body: web::Json<RequestEmailChangeReq>,
) -> Result<HttpResponse> {
    let user = auth_service::request_email_change(&state, claims.user_id, &body.new_email).await?;
    let user_info = user_service::get_self_info(&state, user.id).await?;
    let ctx = audit_service::AuditContext::from_request(&req, &claims);
    audit_service::log(
        &state,
        &ctx,
        audit_service::AuditAction::UserRequestEmailChange,
        Some("user"),
        Some(user.id),
        Some(&user.username),
        None,
    )
    .await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(user_info)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/email/change/resend",
    tag = "auth",
    operation_id = "resend_email_change",
    responses(
        (status = 200, description = "Email change confirmation resend request accepted", body = inline(ApiResponse<ActionMessageResp>)),
        (status = 400, description = "No pending email change"),
    ),
    security(("bearer" = [])),
)]
pub async fn resend_email_change(
    state: web::Data<AppState>,
    req: HttpRequest,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let started_at = tokio::time::Instant::now();
    let result = auth_service::resend_email_change(&state, claims.user_id).await;
    let user = match result {
        Ok(user) => user,
        Err(error) => {
            apply_auth_mail_response_floor(started_at).await;
            return Err(error);
        }
    };
    if let Some(user) = user {
        let ctx = audit_service::AuditContext::from_request(&req, &claims);
        audit_service::log(
            &state,
            &ctx,
            audit_service::AuditAction::UserResendEmailChange,
            Some("user"),
            Some(user.id),
            Some(&user.username),
            None,
        )
        .await;
    }
    apply_auth_mail_response_floor(started_at).await;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(ActionMessageResp {
        message: "If an email change is pending, a confirmation email will be sent".to_string(),
    })))
}

/// Update the current user's preferences.
///
/// Only non-null fields in the request body are merged into the existing
/// preferences. Returns the full updated preferences object.
#[api_docs_macros::path(
    patch,
    path = "/api/v1/auth/preferences",
    tag = "auth",
    operation_id = "update_preferences",
    request_body = UpdatePreferencesReq,
    responses(
        (status = 200, description = "Preferences updated", body = inline(ApiResponse<crate::api::routes::auth::UserPreferences>)),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_preferences(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdatePreferencesReq>,
) -> Result<HttpResponse> {
    let prefs = user_service::update_preferences(&state, claims.user_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(prefs)))
}

#[api_docs_macros::path(
    patch,
    path = "/api/v1/auth/profile",
    tag = "auth",
    operation_id = "update_profile",
    request_body = UpdateProfileReq,
    responses(
        (status = 200, description = "Profile updated", body = inline(ApiResponse<crate::api::routes::auth::UserProfileInfo>)),
        (status = 400, description = "Invalid profile input"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn patch_profile(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdateProfileReq>,
) -> Result<HttpResponse> {
    let profile =
        profile_service::update_profile(&state, claims.user_id, body.display_name.clone()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/auth/profile/avatar/upload",
    tag = "auth",
    operation_id = "upload_avatar",
    request_body(content = String, content_type = "multipart/form-data", description = "Avatar image to upload"),
    responses(
        (status = 200, description = "Avatar uploaded", body = inline(ApiResponse<crate::api::routes::auth::UserProfileInfo>)),
        (status = 400, description = "Invalid image upload"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn upload_avatar(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse> {
    let profile = profile_service::upload_avatar(&state, claims.user_id, &mut payload).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    put,
    path = "/api/v1/auth/profile/avatar/source",
    tag = "auth",
    operation_id = "set_avatar_source",
    request_body = UpdateAvatarSourceReq,
    responses(
        (status = 200, description = "Avatar source updated", body = inline(ApiResponse<crate::api::routes::auth::UserProfileInfo>)),
        (status = 400, description = "Invalid avatar source"),
        (status = 401, description = "Not authenticated"),
    ),
    security(("bearer" = [])),
)]
pub async fn put_avatar_source(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<UpdateAvatarSourceReq>,
) -> Result<HttpResponse> {
    let profile = profile_service::set_avatar_source(&state, claims.user_id, body.source).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(profile)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/auth/profile/avatar/{size}",
    tag = "auth",
    operation_id = "get_self_avatar",
    params(("size" = u32, Path, description = "Avatar size (512 or 1024)")),
    responses(
        (status = 200, description = "Avatar image (WebP)"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Avatar not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn get_self_avatar(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<u32>,
) -> Result<HttpResponse> {
    let bytes = profile_service::get_avatar_bytes(&state, claims.user_id, *path).await?;
    Ok(profile_service::avatar_image_response(bytes))
}
