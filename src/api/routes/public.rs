//! API 路由：`public`。

use crate::api::response::ApiResponse;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::config_service;
use actix_web::{HttpResponse, web};

pub fn routes() -> impl actix_web::dev::HttpServiceFactory + use<> {
    web::scope("/public")
        .route("/branding", web::get().to(get_branding))
        .route("/preview-apps", web::get().to(get_preview_apps))
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
pub async fn get_branding(state: web::Data<AppState>) -> Result<HttpResponse> {
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
pub async fn get_preview_apps(state: web::Data<AppState>) -> Result<HttpResponse> {
    let preview_apps = config_service::get_public_preview_apps(&state);
    Ok(HttpResponse::Ok().json(ApiResponse::ok(preview_apps)))
}
