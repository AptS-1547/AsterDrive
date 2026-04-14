use actix_web::{
    Error, HttpResponse,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::{
        Method,
        header::{self, HeaderMap, HeaderValue},
    },
    web,
};
use futures::future::{LocalBoxFuture, Ready, ok};
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::config::cors::RuntimeCorsPolicy;
use crate::errors::{AsterError, MapAsterErr};
use crate::runtime::AppState;

const ALLOWED_METHODS: &[&str] = &[
    "GET",
    "POST",
    "PUT",
    "PATCH",
    "DELETE",
    "OPTIONS",
    "PROPFIND",
    "PROPPATCH",
    "MKCOL",
    "COPY",
    "MOVE",
    "LOCK",
    "UNLOCK",
];

const ALLOWED_HEADERS: &[&str] = &[
    "authorization",
    "accept",
    "content-type",
    "depth",
    "destination",
    "if",
    "lock-token",
    "overwrite",
    "timeout",
    "x-csrf-token",
    "x-wopi-lock",
    "x-wopi-oldlock",
    "x-wopi-override",
    "x-wopi-overwriterelativetarget",
    "x-wopi-requestedname",
    "x-wopi-relativetarget",
    "x-wopi-size",
    "x-wopi-suggestedtarget",
];

const EXPOSE_HEADERS: &[&str] = &[
    "dav",
    "lock-token",
    "x-wopi-itemversion",
    "x-wopi-invalidfilenameerror",
    "x-wopi-lock",
    "x-wopi-lockfailurereason",
    "x-wopi-validrelativetarget",
];

pub struct RuntimeCors;

impl<S, B> Transform<S, ServiceRequest> for RuntimeCors
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RuntimeCorsMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RuntimeCorsMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct RuntimeCorsMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RuntimeCorsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();

        Box::pin(async move {
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or_else(|| AsterError::internal_error("AppState not found"))?;
            let policy = RuntimeCorsPolicy::from_runtime_config(&state.runtime_config);
            let public_site_origin =
                crate::config::site_url::public_site_url(&state.runtime_config);

            // Static assets and public pages don't need CORS enforcement
            if is_cors_exempt_path(req.path()) {
                let mut response = svc.call(req).await?.map_into_left_body();
                apply_public_site_origin_headers(
                    response.headers_mut(),
                    &policy,
                    public_site_origin.as_deref(),
                )?;
                return Ok(response);
            }

            let Some(origin_header) = req.headers().get(header::ORIGIN).cloned() else {
                let mut response = svc.call(req).await?.map_into_left_body();
                apply_public_site_origin_headers(
                    response.headers_mut(),
                    &policy,
                    public_site_origin.as_deref(),
                )?;
                return Ok(response);
            };

            let origin = crate::config::cors::normalize_origin(
                origin_header
                    .to_str()
                    .map_aster_err_with(|| AsterError::validation_error("invalid Origin header"))?,
                false,
            )?;

            if origin_matches_public_site_url(public_site_origin.as_deref(), &origin) {
                if is_preflight_request(&req) {
                    if !requested_method_is_allowed(&req) || !requested_headers_are_allowed(&req)? {
                        return Ok(forbidden(req).map_into_right_body());
                    }

                    let mut response = HttpResponse::NoContent().finish();
                    apply_origin_headers(response.headers_mut(), &policy, &origin)?;
                    apply_preflight_headers(response.headers_mut(), &policy);
                    return Ok(req.into_response(response).map_into_right_body());
                }

                let mut response = svc.call(req).await?.map_into_left_body();
                apply_origin_headers(response.headers_mut(), &policy, &origin)?;
                apply_actual_headers(response.headers_mut(), &policy);
                return Ok(response);
            }

            if !policy.enforces_requests() {
                let mut response = svc.call(req).await?.map_into_left_body();
                apply_public_site_origin_headers(
                    response.headers_mut(),
                    &policy,
                    public_site_origin.as_deref(),
                )?;
                return Ok(response);
            }

            if request_is_same_origin(&req, &origin) {
                let mut response = svc.call(req).await?.map_into_left_body();
                apply_public_site_origin_headers(
                    response.headers_mut(),
                    &policy,
                    public_site_origin.as_deref(),
                )?;
                return Ok(response);
            }

            if !policy.allows_origin(&origin) {
                return Ok(forbidden(req).map_into_right_body());
            }

            if is_preflight_request(&req) {
                if !requested_method_is_allowed(&req) || !requested_headers_are_allowed(&req)? {
                    return Ok(forbidden(req).map_into_right_body());
                }

                let mut response = HttpResponse::NoContent().finish();
                apply_origin_headers(response.headers_mut(), &policy, &origin)?;
                apply_preflight_headers(response.headers_mut(), &policy);
                return Ok(req.into_response(response).map_into_right_body());
            }

            let mut response = svc.call(req).await?.map_into_left_body();
            apply_origin_headers(response.headers_mut(), &policy, &origin)?;
            apply_actual_headers(response.headers_mut(), &policy);
            Ok(response)
        })
    }
}

fn is_preflight_request(req: &ServiceRequest) -> bool {
    req.method() == Method::OPTIONS
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
}

/// Paths that serve static assets or public pages — no CORS enforcement needed.
fn is_cors_exempt_path(path: &str) -> bool {
    matches!(
        path,
        "/" | "/index.html" | "/favicon.svg" | "/manifest.webmanifest" | "/sw.js"
    ) || path.starts_with("/workbox-")
        || path.starts_with("/assets/")
        || path.starts_with("/static/")
        || path.starts_with("/pdfjs/")
}

fn request_is_same_origin(req: &ServiceRequest, origin: &str) -> bool {
    let conn = req.connection_info();
    let request_origin = format!(
        "{}://{}",
        conn.scheme().to_ascii_lowercase(),
        conn.host().to_ascii_lowercase()
    );
    request_origin == origin
}

fn requested_method_is_allowed(req: &ServiceRequest) -> bool {
    let Some(method) = req.headers().get(header::ACCESS_CONTROL_REQUEST_METHOD) else {
        return false;
    };

    let Ok(method) = method.to_str() else {
        return false;
    };

    ALLOWED_METHODS.contains(&method)
}

fn requested_headers_are_allowed(req: &ServiceRequest) -> Result<bool, AsterError> {
    let Some(request_headers) = req.headers().get(header::ACCESS_CONTROL_REQUEST_HEADERS) else {
        return Ok(true);
    };

    let request_headers = request_headers.to_str().map_aster_err_with(|| {
        AsterError::validation_error("invalid Access-Control-Request-Headers")
    })?;

    let allowed_headers = ALLOWED_HEADERS
        .iter()
        .copied()
        .collect::<BTreeSet<&'static str>>();

    for requested in request_headers.split(',') {
        let requested = requested.trim().to_ascii_lowercase();
        if requested.is_empty() {
            continue;
        }

        let _: header::HeaderName = requested.parse().map_aster_err_with(|| {
            AsterError::validation_error("invalid Access-Control-Request-Headers")
        })?;

        if !allowed_headers.contains(requested.as_str()) {
            return Ok(false);
        }
    }

    Ok(true)
}

fn origin_matches_public_site_url(public_site_origin: Option<&str>, origin: &str) -> bool {
    public_site_origin == Some(origin)
}

fn apply_origin_headers(
    headers: &mut HeaderMap,
    policy: &RuntimeCorsPolicy,
    origin: &str,
) -> Result<(), AsterError> {
    if !headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN) {
        let value = if policy.sends_wildcard_origin() {
            HeaderValue::from_static("*")
        } else {
            HeaderValue::from_str(origin).map_aster_err_with(|| {
                AsterError::internal_error("failed to serialize Access-Control-Allow-Origin")
            })?
        };

        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }

    if policy.allow_credentials && !headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS) {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }

    ensure_vary(headers, "Origin")?;
    Ok(())
}

fn apply_preflight_headers(headers: &mut HeaderMap, policy: &RuntimeCorsPolicy) {
    let allow_headers = ALLOWED_HEADERS.join(", ");

    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(
            "GET, POST, PUT, PATCH, DELETE, OPTIONS, PROPFIND, PROPPATCH, MKCOL, COPY, MOVE, LOCK, UNLOCK",
        ),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_str(&allow_headers)
            .expect("CORS allow headers should always be a valid header value"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_str(&policy.max_age_secs.to_string())
            .expect("CORS max age should always be a valid header value"),
    );
    ensure_vary(headers, "Access-Control-Request-Method").ok();
    ensure_vary(headers, "Access-Control-Request-Headers").ok();
}

fn apply_actual_headers(headers: &mut HeaderMap, _policy: &RuntimeCorsPolicy) {
    let expose_headers = EXPOSE_HEADERS.join(", ");
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_str(&expose_headers)
            .expect("CORS expose headers should always be a valid header value"),
    );
}

fn apply_public_site_origin_headers(
    headers: &mut HeaderMap,
    policy: &RuntimeCorsPolicy,
    public_site_origin: Option<&str>,
) -> Result<(), AsterError> {
    if !policy.enabled {
        return Ok(());
    }

    let Some(public_site_origin) = public_site_origin else {
        return Ok(());
    };

    apply_origin_headers(headers, policy, public_site_origin)
}

fn ensure_vary(headers: &mut HeaderMap, value: &str) -> Result<(), AsterError> {
    let mut vary_values = BTreeSet::new();

    if let Some(existing) = headers.get(header::VARY) {
        let existing = existing
            .to_str()
            .map_aster_err_ctx("invalid Vary header", AsterError::internal_error)?;
        for item in existing.split(',') {
            let item = item.trim();
            if !item.is_empty() {
                vary_values.insert(item.to_string());
            }
        }
    }

    vary_values.insert(value.to_string());
    let joined = vary_values.into_iter().collect::<Vec<_>>().join(", ");
    let header_value = HeaderValue::from_str(&joined).map_aster_err_ctx(
        "failed to serialize Vary header",
        AsterError::internal_error,
    )?;
    headers.insert(header::VARY, header_value);
    Ok(())
}

fn forbidden(req: ServiceRequest) -> ServiceResponse {
    let mut response = HttpResponse::Forbidden().finish();
    let _ = ensure_vary(response.headers_mut(), "Origin");
    let _ = ensure_vary(response.headers_mut(), "Access-Control-Request-Method");
    let _ = ensure_vary(response.headers_mut(), "Access-Control-Request-Headers");
    req.into_response(response)
}

#[cfg(test)]
mod tests {
    use super::{
        ALLOWED_HEADERS, ALLOWED_METHODS, RuntimeCors, apply_origin_headers,
        apply_public_site_origin_headers, ensure_vary, is_cors_exempt_path,
        origin_matches_public_site_url, request_is_same_origin, requested_headers_are_allowed,
        requested_method_is_allowed,
    };
    use crate::cache;
    use crate::config::cors::{CorsAllowedOrigins, RuntimeCorsPolicy};
    use crate::config::{CacheConfig, Config, DatabaseConfig, RuntimeConfig};
    use crate::entities::system_config;
    use crate::runtime::AppState;
    use actix_web::{
        App, HttpResponse,
        http::header::{self, HeaderMap, HeaderValue},
        test as actix_test, web,
    };
    use chrono::Utc;
    use std::sync::Arc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 0,
            key: key.to_string(),
            value: value.to_string(),
            value_type: "string".to_string(),
            requires_restart: false,
            is_sensitive: false,
            source: "system".to_string(),
            namespace: String::new(),
            category: "test".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn test_policy(
        enabled: bool,
        allowed_origins: CorsAllowedOrigins,
        allow_credentials: bool,
    ) -> RuntimeCorsPolicy {
        RuntimeCorsPolicy {
            enabled,
            allowed_origins,
            allow_credentials,
            max_age_secs: 600,
        }
    }

    async fn test_state(configs: &[(&str, &str)]) -> AppState {
        let db = crate::db::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            pool_size: 1,
            retry_count: 0,
        })
        .await
        .unwrap();

        let runtime_config = Arc::new(RuntimeConfig::new());
        for (key, value) in configs {
            runtime_config.apply(config_model(key, value));
        }

        let cache = cache::create_cache(&CacheConfig {
            enabled: false,
            ..Default::default()
        })
        .await;
        let (thumbnail_tx, _thumbnail_rx) = tokio::sync::mpsc::channel::<i64>(1);
        let (storage_change_tx, _) = tokio::sync::broadcast::channel(
            crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
        );

        AppState {
            db,
            driver_registry: Arc::new(crate::storage::DriverRegistry::new()),
            runtime_config: runtime_config.clone(),
            policy_snapshot: Arc::new(crate::storage::PolicySnapshot::new()),
            config: Arc::new(Config::default()),
            cache,
            mail_sender: crate::services::mail_service::runtime_sender(runtime_config.clone()),
            thumbnail_tx,
            storage_change_tx,
        }
    }

    #[test]
    fn cors_exempt_paths_cover_static_assets_and_manifest() {
        for path in [
            "/",
            "/index.html",
            "/favicon.svg",
            "/manifest.webmanifest",
            "/sw.js",
            "/workbox-abc123.js",
            "/assets/app.js",
            "/static/logo.png",
            "/pdfjs/viewer.js",
        ] {
            assert!(
                is_cors_exempt_path(path),
                "{path} should bypass CORS checks"
            );
        }

        assert!(!is_cors_exempt_path("/api/v1/auth/check"));
        assert!(!is_cors_exempt_path("/manifest.json"));
    }

    #[test]
    fn public_site_origin_matching_is_exact() {
        assert!(origin_matches_public_site_url(
            Some("https://drive.example.com"),
            "https://drive.example.com",
        ));
        assert!(!origin_matches_public_site_url(
            Some("https://drive.example.com"),
            "https://cdn.example.com",
        ));
        assert!(!origin_matches_public_site_url(
            None,
            "https://drive.example.com",
        ));
    }

    #[actix_web::test]
    async fn request_same_origin_matches_scheme_and_host_case_insensitively() {
        let req = actix_test::TestRequest::get()
            .uri("/health")
            .insert_header((header::HOST, "Drive.EXAMPLE.com:8443"))
            .to_srv_request();

        assert!(request_is_same_origin(
            &req,
            "http://drive.example.com:8443",
        ));
        assert!(!request_is_same_origin(
            &req,
            "https://drive.example.com:8443",
        ));
    }

    #[actix_web::test]
    async fn requested_method_validation_accepts_known_and_rejects_unknown_methods() {
        let req = actix_test::TestRequest::default()
            .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
            .to_srv_request();
        assert!(requested_method_is_allowed(&req));

        let req = actix_test::TestRequest::default()
            .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "TRACE"))
            .to_srv_request();
        assert!(!requested_method_is_allowed(&req));

        assert!(ALLOWED_METHODS.contains(&"PROPFIND"));
        assert!(ALLOWED_METHODS.contains(&"LOCK"));
    }

    #[actix_web::test]
    async fn requested_headers_validation_accepts_known_headers_case_insensitively() {
        let req = actix_test::TestRequest::default()
            .insert_header((
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                "Authorization, Content-Type, LOCK-TOKEN, X-WOPI-Override, X-WOPI-Lock",
            ))
            .to_srv_request();

        assert!(requested_headers_are_allowed(&req).unwrap());
        assert!(ALLOWED_HEADERS.contains(&"authorization"));
        assert!(ALLOWED_HEADERS.contains(&"lock-token"));
        assert!(ALLOWED_HEADERS.contains(&"x-wopi-override"));
        assert!(ALLOWED_HEADERS.contains(&"x-wopi-lock"));
    }

    #[actix_web::test]
    async fn requested_headers_validation_rejects_unknown_header_names() {
        let req = actix_test::TestRequest::default()
            .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "x-custom-header"))
            .to_srv_request();

        assert!(!requested_headers_are_allowed(&req).unwrap());
    }

    #[actix_web::test]
    async fn requested_headers_validation_rejects_invalid_header_syntax() {
        let req = actix_test::TestRequest::default()
            .insert_header((header::ACCESS_CONTROL_REQUEST_HEADERS, "bad header"))
            .to_srv_request();

        let err = requested_headers_are_allowed(&req).unwrap_err();
        assert!(err.message().contains("Access-Control-Request-Headers"));
    }

    #[test]
    fn apply_origin_headers_sets_origin_credentials_and_vary() {
        let policy = test_policy(
            true,
            CorsAllowedOrigins::List(vec!["https://drive.example.com".to_string()]),
            true,
        );
        let mut headers = HeaderMap::new();

        apply_origin_headers(&mut headers, &policy, "https://drive.example.com").unwrap();

        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://drive.example.com"
        );
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap()
                .to_str()
                .unwrap(),
            "true"
        );
        assert!(
            headers
                .get(header::VARY)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Origin")
        );
    }

    #[test]
    fn apply_origin_headers_preserves_existing_allow_origin_header() {
        let policy = test_policy(true, CorsAllowedOrigins::Any, true);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("https://existing.example.com"),
        );

        apply_origin_headers(&mut headers, &policy, "https://drive.example.com").unwrap();

        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://existing.example.com"
        );
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap()
                .to_str()
                .unwrap(),
            "true"
        );
    }

    #[test]
    fn apply_public_site_origin_headers_noop_when_disabled_or_origin_missing() {
        let mut headers = HeaderMap::new();
        apply_public_site_origin_headers(
            &mut headers,
            &test_policy(false, CorsAllowedOrigins::None, false),
            Some("https://drive.example.com"),
        )
        .unwrap();
        assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

        apply_public_site_origin_headers(
            &mut headers,
            &test_policy(true, CorsAllowedOrigins::None, false),
            None,
        )
        .unwrap();
        assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    }

    #[test]
    fn apply_public_site_origin_headers_uses_public_site_url_and_respects_existing_header() {
        let mut headers = HeaderMap::new();
        apply_public_site_origin_headers(
            &mut headers,
            &test_policy(true, CorsAllowedOrigins::None, false),
            Some("https://drive.example.com"),
        )
        .unwrap();
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://drive.example.com"
        );

        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("https://existing.example.com"),
        );
        apply_public_site_origin_headers(
            &mut headers,
            &test_policy(true, CorsAllowedOrigins::None, false),
            Some("https://drive.example.com"),
        )
        .unwrap();
        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://existing.example.com"
        );
    }

    #[test]
    fn ensure_vary_deduplicates_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::VARY,
            HeaderValue::from_static("Origin, Access-Control-Request-Method"),
        );

        ensure_vary(&mut headers, "Origin").unwrap();
        ensure_vary(&mut headers, "Access-Control-Request-Headers").unwrap();

        assert_eq!(
            headers.get(header::VARY).unwrap().to_str().unwrap(),
            "Access-Control-Request-Headers, Access-Control-Request-Method, Origin"
        );
    }

    #[actix_web::test]
    async fn middleware_allows_public_site_origin_preflight_without_whitelist() {
        let state = test_state(&[
            ("cors_enabled", "true"),
            ("public_site_url", "https://drive.example.com"),
        ])
        .await;
        let app = actix_test::init_service(
            App::new()
                .wrap(RuntimeCors)
                .app_data(web::Data::new(state))
                .route(
                    "/health",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;

        let req = actix_test::TestRequest::default()
            .method(actix_web::http::Method::OPTIONS)
            .uri("/health")
            .insert_header((header::HOST, "internal.example.local"))
            .insert_header((header::ORIGIN, "https://drive.example.com"))
            .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status(), 204);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://drive.example.com"
        );
    }

    #[actix_web::test]
    async fn middleware_adds_public_site_origin_to_passthrough_response_without_origin_header() {
        let state = test_state(&[
            ("cors_enabled", "true"),
            ("public_site_url", "https://drive.example.com"),
        ])
        .await;
        let app = actix_test::init_service(
            App::new()
                .wrap(RuntimeCors)
                .app_data(web::Data::new(state))
                .route(
                    "/health",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/health").to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://drive.example.com"
        );
    }

    #[actix_web::test]
    async fn middleware_does_not_override_existing_allow_origin_header() {
        let state = test_state(&[
            ("cors_enabled", "true"),
            ("public_site_url", "https://drive.example.com"),
        ])
        .await;
        let app = actix_test::init_service(
            App::new()
                .wrap(RuntimeCors)
                .app_data(web::Data::new(state))
                .route(
                    "/custom",
                    web::get().to(|| async {
                        HttpResponse::Ok()
                            .insert_header((
                                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                                "https://existing.example.com",
                            ))
                            .finish()
                    }),
                ),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/custom").to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            "https://existing.example.com"
        );
    }
}
