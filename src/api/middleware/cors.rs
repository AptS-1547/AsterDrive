use actix_cors::Cors;
use actix_web::http;

use crate::api::constants::HOUR_SECS;

/// 返回 CORS 配置，包含标准 HTTP 和 WebDAV 方法/头部
pub fn configure_cors() -> Cors {
    Cors::default()
        .allow_any_origin()
        .allowed_methods(vec![
            "GET",
            "POST",
            "PUT",
            "PATCH",
            "DELETE",
            "OPTIONS",
            // WebDAV methods
            "PROPFIND",
            "PROPPATCH",
            "MKCOL",
            "COPY",
            "MOVE",
            "LOCK",
            "UNLOCK",
        ])
        .allowed_headers(vec![
            http::header::AUTHORIZATION,
            http::header::ACCEPT,
            http::header::CONTENT_TYPE,
            // WebDAV headers
            http::header::HeaderName::from_static("depth"),
            http::header::HeaderName::from_static("destination"),
            http::header::HeaderName::from_static("if"),
            http::header::HeaderName::from_static("lock-token"),
            http::header::HeaderName::from_static("overwrite"),
            http::header::HeaderName::from_static("timeout"),
        ])
        .expose_headers(vec![
            http::header::HeaderName::from_static("dav"),
            http::header::HeaderName::from_static("lock-token"),
        ])
        .max_age(HOUR_SECS as usize)
}
