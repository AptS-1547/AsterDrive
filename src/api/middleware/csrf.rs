use actix_web::{
    HttpRequest,
    dev::ServiceRequest,
    http::{Method, header},
};
use http::Uri;
use rand::RngExt;

use crate::config::{RuntimeConfig, cors, site_url};
use crate::errors::{AsterError, MapAsterErr, Result};

pub const CSRF_COOKIE: &str = "aster_csrf";
pub const CSRF_HEADER: &str = "X-CSRF-Token";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSourceMode {
    OptionalWhenPresent,
    Required,
}

pub fn is_unsafe_method(method: &Method) -> bool {
    !matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

pub fn ensure_request_source_allowed(
    req: &HttpRequest,
    runtime_config: &RuntimeConfig,
    mode: RequestSourceMode,
) -> Result<()> {
    let conn = req.connection_info();
    let request_origin = request_origin(conn.scheme(), conn.host())?;
    ensure_headers_allowed(
        header_value(req, header::ORIGIN),
        header_value(req, header::REFERER),
        header_value(req, header::HeaderName::from_static("sec-fetch-site")),
        &request_origin,
        site_url::public_site_url(runtime_config).as_deref(),
        mode,
    )
}

pub fn ensure_service_request_source_allowed(
    req: &ServiceRequest,
    runtime_config: &RuntimeConfig,
    mode: RequestSourceMode,
) -> Result<()> {
    let conn = req.connection_info();
    let request_origin = request_origin(conn.scheme(), conn.host())?;
    ensure_headers_allowed(
        header_value(req.request(), header::ORIGIN),
        header_value(req.request(), header::REFERER),
        header_value(
            req.request(),
            header::HeaderName::from_static("sec-fetch-site"),
        ),
        &request_origin,
        site_url::public_site_url(runtime_config).as_deref(),
        mode,
    )
}

pub fn build_csrf_token() -> String {
    use base64::Engine;

    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn ensure_double_submit_token(req: &HttpRequest) -> Result<()> {
    let cookie_token = req
        .cookie(CSRF_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| AsterError::auth_forbidden("missing CSRF cookie"))?;
    let header_token = header_value(req, header::HeaderName::from_static("x-csrf-token"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AsterError::auth_forbidden("missing X-CSRF-Token header"))?;

    if header_token != cookie_token {
        return Err(AsterError::auth_forbidden("invalid CSRF token"));
    }

    Ok(())
}

pub fn ensure_service_double_submit_token(req: &ServiceRequest) -> Result<()> {
    ensure_double_submit_token(req.request())
}

fn ensure_headers_allowed(
    origin: Option<&str>,
    referer: Option<&str>,
    sec_fetch_site: Option<&str>,
    request_origin: &str,
    public_site_origin: Option<&str>,
    mode: RequestSourceMode,
) -> Result<()> {
    if let Some(fetch_site) = sec_fetch_site
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
    {
        match fetch_site.as_str() {
            "same-origin" | "none" => {}
            "same-site" | "cross-site" => {
                return Err(AsterError::auth_forbidden(
                    "untrusted request source for cookie-authenticated action",
                ));
            }
            _ => {}
        }
    }

    if let Some(origin) = origin
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| cors::normalize_origin(value, false))
        .transpose()
        .map_aster_err_with(|| AsterError::validation_error("invalid Origin header"))?
    {
        if origin_is_trusted(&origin, request_origin, public_site_origin) {
            return Ok(());
        }
        return Err(AsterError::auth_forbidden(
            "untrusted request origin for cookie-authenticated action",
        ));
    }

    if let Some(referer) = referer.map(str::trim).filter(|value| !value.is_empty()) {
        let referer_origin = origin_from_url(referer)
            .ok_or_else(|| AsterError::validation_error("invalid Referer header"))?;
        if origin_is_trusted(&referer_origin, request_origin, public_site_origin) {
            return Ok(());
        }
        return Err(AsterError::auth_forbidden(
            "untrusted request referer for cookie-authenticated action",
        ));
    }

    match mode {
        RequestSourceMode::OptionalWhenPresent => Ok(()),
        RequestSourceMode::Required => Err(AsterError::auth_forbidden(
            "missing request source for cookie-authenticated action",
        )),
    }
}

fn header_value(req: &HttpRequest, name: header::HeaderName) -> Option<&str> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

fn request_origin(scheme: &str, host: &str) -> Result<String> {
    cors::normalize_origin(&format!("{scheme}://{host}"), false)
        .map_aster_err_with(|| AsterError::validation_error("invalid request host"))
}

fn origin_is_trusted(origin: &str, request_origin: &str, public_site_origin: Option<&str>) -> bool {
    origin == request_origin || public_site_origin.is_some_and(|allowed| allowed == origin)
}

fn origin_from_url(url: &str) -> Option<String> {
    let uri: Uri = url.parse().ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    let host = uri.host()?.to_ascii_lowercase();
    let port = uri
        .port_u16()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    cors::normalize_origin(&format!("{scheme}://{host}{port}"), false).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        CSRF_COOKIE, CSRF_HEADER, RequestSourceMode, build_csrf_token, ensure_double_submit_token,
        ensure_headers_allowed,
    };
    use actix_web::cookie::Cookie;

    #[test]
    fn accepts_same_origin_and_public_site_origin() {
        assert!(
            ensure_headers_allowed(
                Some("http://localhost"),
                None,
                Some("same-origin"),
                "http://localhost",
                Some("https://drive.example.com"),
                RequestSourceMode::Required,
            )
            .is_ok()
        );

        assert!(
            ensure_headers_allowed(
                Some("https://drive.example.com"),
                None,
                Some("same-origin"),
                "http://127.0.0.1:3000",
                Some("https://drive.example.com"),
                RequestSourceMode::Required,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_same_site_and_cross_site_fetch_metadata() {
        let err = ensure_headers_allowed(
            None,
            None,
            Some("same-site"),
            "https://drive.example.com",
            None,
            RequestSourceMode::OptionalWhenPresent,
        )
        .unwrap_err();
        assert!(err.message().contains("untrusted request source"));

        let err = ensure_headers_allowed(
            None,
            None,
            Some("cross-site"),
            "https://drive.example.com",
            None,
            RequestSourceMode::OptionalWhenPresent,
        )
        .unwrap_err();
        assert!(err.message().contains("untrusted request source"));
    }

    #[test]
    fn rejects_untrusted_origin_and_missing_required_source() {
        let err = ensure_headers_allowed(
            Some("https://evil.example.com"),
            None,
            None,
            "https://drive.example.com",
            None,
            RequestSourceMode::OptionalWhenPresent,
        )
        .unwrap_err();
        assert!(err.message().contains("untrusted request origin"));

        let err = ensure_headers_allowed(
            None,
            None,
            None,
            "https://drive.example.com",
            None,
            RequestSourceMode::Required,
        )
        .unwrap_err();
        assert!(err.message().contains("missing request source"));
    }

    #[test]
    fn accepts_missing_optional_source() {
        assert!(
            ensure_headers_allowed(
                None,
                None,
                None,
                "https://drive.example.com",
                None,
                RequestSourceMode::OptionalWhenPresent,
            )
            .is_ok()
        );
    }

    #[test]
    fn build_csrf_token_returns_url_safe_random_value() {
        let token_a = build_csrf_token();
        let token_b = build_csrf_token();

        assert_ne!(token_a, token_b);
        assert!(token_a.len() >= 32);
        assert!(
            token_a
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
    }

    #[test]
    fn csrf_token_check_requires_cookie_for_cookie_authenticated_writes() {
        let req = actix_web::test::TestRequest::post()
            .uri("/api/v1/auth/profile")
            .to_http_request();

        let err = ensure_double_submit_token(&req).unwrap_err();
        assert!(err.message().contains("missing CSRF cookie"));
    }

    #[test]
    fn csrf_token_check_requires_matching_cookie_and_header() {
        let req = actix_web::test::TestRequest::patch()
            .uri("/api/v1/auth/profile")
            .insert_header(("Origin", "http://localhost"))
            .cookie(Cookie::new(CSRF_COOKIE, "token-a"))
            .insert_header((CSRF_HEADER, "token-a"))
            .to_http_request();
        assert!(ensure_double_submit_token(&req).is_ok());

        let missing_header = actix_web::test::TestRequest::patch()
            .uri("/api/v1/auth/profile")
            .insert_header(("Origin", "http://localhost"))
            .cookie(Cookie::new(CSRF_COOKIE, "token-a"))
            .to_http_request();
        let err = ensure_double_submit_token(&missing_header).unwrap_err();
        assert!(err.message().contains("missing X-CSRF-Token"));

        let mismatch = actix_web::test::TestRequest::patch()
            .uri("/api/v1/auth/profile")
            .insert_header(("Origin", "http://localhost"))
            .cookie(Cookie::new(CSRF_COOKIE, "token-a"))
            .insert_header((CSRF_HEADER, "token-b"))
            .to_http_request();
        let err = ensure_double_submit_token(&mismatch).unwrap_err();
        assert!(err.message().contains("invalid CSRF token"));
    }
}
