use crate::api::response::ApiResponse;
use crate::runtime::AppState;
use actix_web::{HttpResponse, web};

pub fn routes() -> actix_web::Scope {
    web::scope("/health")
        .route("", web::get().to(health))
        .route("", web::head().to(health))
        .route("/ready", web::get().to(ready))
        .route("/ready", web::head().to(ready))
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "build_time": compile_time(),
    })))
}

async fn ready(state: web::Data<AppState>) -> HttpResponse {
    match state.db.ping().await {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
            "status": "ready",
            "version": env!("CARGO_PKG_VERSION"),
            "build_time": compile_time(),
        }))),
        Err(e) => HttpResponse::ServiceUnavailable()
            .json(ApiResponse::<()>::error(
                crate::api::error_code::ErrorCode::DatabaseError,
                &e.to_string(),
            )),
    }
}

fn compile_time() -> &'static str {
    // build.rs 里设置的环境变量
    option_env!("ASTER_BUILD_TIME").unwrap_or("unknown")
}
