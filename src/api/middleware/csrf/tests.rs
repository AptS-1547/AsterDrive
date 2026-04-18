//! CSRF 中间件测试。

use actix_web::cookie::Cookie;

use super::source::ensure_headers_allowed;
use super::{
    CSRF_COOKIE, CSRF_HEADER, RequestSourceMode, build_csrf_token, ensure_double_submit_token,
};

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
