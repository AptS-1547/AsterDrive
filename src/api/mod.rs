pub mod error_code;
pub mod middleware;
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
            // /api/v1 下的 404 兜底
            .default_service(web::to(api_not_found)),
    )
    .service(routes::health::routes())
    // frontend 最后注册，兜底所有未匹配路由
    .service(routes::frontend::routes());
}

async fn api_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(
        ErrorCode::EndpointNotFound,
        "endpoint not found",
    ))
}
