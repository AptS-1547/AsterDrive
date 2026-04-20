//! API 路由：`public`。

use crate::api::dto::validate_request;
use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::services::{config_service, remote_enrollment_service};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RedeemRemoteEnrollmentReq {
    #[validate(custom(function = "crate::api::dto::validation::validate_non_blank"))]
    pub token: String,
}

#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AckRemoteEnrollmentReq {
    #[validate(custom(function = "crate::api::dto::validation::validate_non_blank"))]
    pub ack_token: String,
}

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/public")
        .route("/branding", web::get().to(get_branding))
        .route("/preview-apps", web::get().to(get_preview_apps))
        .route(
            "/remote-enrollment/redeem",
            web::post().to(redeem_remote_enrollment),
        )
        .route(
            "/remote-enrollment/ack",
            web::post().to(ack_remote_enrollment),
        )
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/public/branding",
    tag = "public",
    operation_id = "get_public_branding",
    responses(
        (status = 200, description = "Public branding config", body = inline(ApiResponse<config_service::PublicBranding>)),
    ),
)]
pub async fn get_branding(state: web::Data<PrimaryAppState>) -> Result<HttpResponse> {
    let branding = config_service::get_public_branding(&state);
    Ok(HttpResponse::Ok().json(ApiResponse::ok(branding)))
}

#[api_docs_macros::path(
    get,
    path = "/api/v1/public/preview-apps",
    tag = "public",
    operation_id = "get_public_preview_apps",
    responses(
        (status = 200, description = "Public preview app config", body = inline(ApiResponse<crate::services::preview_app_service::PublicPreviewAppsConfig>)),
    ),
)]
pub async fn get_preview_apps(state: web::Data<PrimaryAppState>) -> Result<HttpResponse> {
    let preview_apps = config_service::get_public_preview_apps(&state);
    Ok(HttpResponse::Ok().json(ApiResponse::ok(preview_apps)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/public/remote-enrollment/redeem",
    tag = "public",
    operation_id = "redeem_remote_enrollment",
    request_body = RedeemRemoteEnrollmentReq,
    responses(
        (status = 200, description = "Redeem a remote enrollment token", body = ApiResponse<remote_enrollment_service::RemoteEnrollmentBootstrap>),
    ),
)]
pub async fn redeem_remote_enrollment(
    state: web::Data<PrimaryAppState>,
    body: web::Json<RedeemRemoteEnrollmentReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    let bootstrap = remote_enrollment_service::redeem_enrollment_token(&state, &body.token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::ok(bootstrap)))
}

#[api_docs_macros::path(
    post,
    path = "/api/v1/public/remote-enrollment/ack",
    tag = "public",
    operation_id = "ack_remote_enrollment",
    request_body = AckRemoteEnrollmentReq,
    responses(
        (status = 200, description = "Acknowledge a redeemed remote enrollment session"),
    ),
)]
pub async fn ack_remote_enrollment(
    state: web::Data<PrimaryAppState>,
    body: web::Json<AckRemoteEnrollmentReq>,
) -> Result<HttpResponse> {
    validate_request(&*body)?;
    remote_enrollment_service::ack_enrollment_token(&state, &body.ack_token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::ok_empty()))
}
