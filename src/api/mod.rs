pub mod error_code;
pub mod middleware;
pub mod openapi;
pub mod response;
pub mod routes;

use actix_web::{HttpResponse, web};
use error_code::ErrorCode;
use response::ApiResponse;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(routes::auth::routes())
            .service(routes::files::routes())
            .service(routes::folders::routes())
            .service(routes::admin::routes())
            .service(routes::shares::routes())
            .service(routes::share_public::routes())
            .service(routes::webdav_accounts::routes())
            .service(routes::trash::routes())
            .default_service(web::to(api_not_found)),
    )
    .service(routes::health::routes());

    // OpenAPI + Swagger UI — 仅 debug 构建
    #[cfg(debug_assertions)]
    {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        let spec = openapi::ApiDoc::openapi();
        let spec_clone = spec.clone();
        cfg.service(web::scope("/api-docs").route(
            "/openapi.json",
            web::get().to(move || {
                let s = spec_clone.clone();
                async move { HttpResponse::Ok().json(s) }
            }),
        ));
        cfg.service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", spec));
    }

    // WebDAV — 在 frontend fallback 之前注册
    if let Some(config) = crate::config::try_get_config() {
        crate::webdav::configure(cfg, &config.webdav);
    }

    // frontend 最后注册，兜底所有未匹配路由
    cfg.service(routes::frontend::routes());
}

async fn api_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(
        ErrorCode::EndpointNotFound,
        "endpoint not found",
    ))
}
