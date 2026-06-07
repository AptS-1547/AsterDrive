use super::api_error_code::ApiErrorCode;
use super::response::ApiResponse;
use actix_web::HttpResponse;

pub(super) async fn api_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error(
        ApiErrorCode::EndpointNotFound,
        "endpoint not found",
    ))
}
