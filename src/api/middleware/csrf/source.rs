use actix_web::{
    HttpRequest,
    dev::ServiceRequest,
    http::{Method, header},
};
use http::Uri;

use crate::config::{RuntimeConfig, cors, site_url};
use crate::errors::{AsterError, MapAsterErr, Result};

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

pub(super) fn ensure_headers_allowed(
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
