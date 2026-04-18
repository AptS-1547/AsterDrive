//! API 路由：`health`。

use crate::api::response::{ApiResponse, HealthResponse, MemoryStatsResponse};
use crate::runtime::AppState;
use actix_web::{HttpResponse, web};

const READY_DB_UNAVAILABLE_MESSAGE: &str = "Database unavailable";
#[cfg(feature = "metrics")]
const METRICS_EXPORT_FAILED_MESSAGE: &str = "Failed to export metrics";

pub fn routes() -> actix_web::Scope {
    let scope = web::scope("/health")
        .route("", web::get().to(health))
        .route("", web::head().to(health))
        .route("/ready", web::get().to(ready))
        .route("/ready", web::head().to(ready));

    #[cfg(all(debug_assertions, feature = "openapi"))]
    let scope = scope.route("/memory", web::get().to(memory));

    #[cfg(feature = "metrics")]
    let scope = scope.route("/metrics", web::get().to(metrics_endpoint));

    scope
}

#[api_docs_macros::path(
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

#[api_docs_macros::path(
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
        Err(e) => {
            tracing::error!(error = %e, "health readiness database ping failed");
            HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
                crate::api::error_code::ErrorCode::DatabaseError,
                READY_DB_UNAVAILABLE_MESSAGE,
            ))
        }
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
        Err(e) => {
            tracing::error!(error = %e, "metrics export failed");
            HttpResponse::InternalServerError().body(METRICS_EXPORT_FAILED_MESSAGE)
        }
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
