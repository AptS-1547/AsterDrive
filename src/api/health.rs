use actix_web::HttpResponse;
use serde_json::json;

/// Health check endpoint
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "healthy",
        "service": "AsterDrive",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
