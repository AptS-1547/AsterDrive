use super::error_code::ErrorCode;
use super::response::ApiResponse;
use actix_web::HttpResponse;

pub(super) async fn api_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(
        ErrorCode::EndpointNotFound,
        "endpoint not found",
    ))
}
