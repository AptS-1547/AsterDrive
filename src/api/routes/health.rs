use crate::api::response::{ApiResponse, HealthResponse, MemoryStatsResponse};
use crate::runtime::AppState;
use actix_web::{HttpResponse, web};

pub fn routes() -> actix_web::Scope {
    let scope = web::scope("/health")
        .route("", web::get().to(health))
        .route("", web::head().to(health))
        .route("/ready", web::get().to(ready))
        .route("/ready", web::head().to(ready));

    #[cfg(debug_assertions)]
    let scope = scope.route("/memory", web::get().to(memory));

    #[cfg(feature = "metrics")]
    let scope = scope.route("/metrics", web::get().to(metrics_endpoint));

    scope
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    operation_id = "health",
    responses(
        (status = 200, description = "Service is healthy", body = inline(ApiResponse<crate::api::response::HealthResponse>)),
    ),
)]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(status_response("ok")))
}

#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    operation_id = "ready",
    responses(
        (status = 200, description = "Service is ready", body = inline(ApiResponse<crate::api::response::HealthResponse>)),
        (status = 503, description = "Service unavailable"),
    ),
)]
pub async fn ready(state: web::Data<AppState>) -> HttpResponse {
    match state.db.ping().await {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(status_response("ready"))),
        Err(e) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            crate::api::error_code::ErrorCode::DatabaseError,
            &e.to_string(),
        )),
    }
}

pub async fn memory() -> HttpResponse {
    let (allocated, peak) = crate::alloc::stats();
    HttpResponse::Ok().json(ApiResponse::ok(MemoryStatsResponse {
        heap_allocated_mb: format!("{allocated:.2}"),
        heap_peak_mb: format!("{peak:.2}"),
    }))
}

#[cfg(feature = "metrics")]
pub async fn metrics_endpoint() -> HttpResponse {
    let Some(metrics) = crate::metrics::get_metrics() else {
        return HttpResponse::ServiceUnavailable()
            .content_type("text/plain")
            .body("Metrics not initialized");
    };

    // 更新 uptime
    metrics.uptime_seconds.set(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
    );

    match metrics.export() {
        Ok(output) => HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4; charset=utf-8")
            .body(output),
        Err(e) => HttpResponse::InternalServerError().body(format!("Failed to export: {e}")),
    }
}

#[inline]
fn compile_time() -> &'static str {
    option_env!("ASTER_BUILD_TIME").unwrap_or("unknown")
}

fn status_response(status: &str) -> HealthResponse {
    HealthResponse {
        status: status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_time: compile_time().to_string(),
    }
}
