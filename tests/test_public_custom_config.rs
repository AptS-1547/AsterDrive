//! 集成测试：`public_custom_config`。

#[macro_use]
mod common;

use actix_web::test;
use serde_json::{Value, json};

async fn set_custom_config(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: &str,
    key: &str,
    value: &str,
    visibility: &str,
) -> Value {
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/admin/config/{key}"))
        .insert_header(("Cookie", common::access_cookie_header(token)))
        .insert_header(common::csrf_header_for(token))
        .set_json(json!({
            "value": value,
            "visibility": visibility,
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status(), 200, "setting {key} should succeed");
    test::read_body_json(resp).await
}

#[actix_web::test]
async fn test_public_custom_config_filters_by_visibility_and_authentication() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let public_body = set_custom_config(
        &app,
        &token,
        "custom.public_theme",
        "nebula",
        "public",
    )
    .await;
    assert_eq!(public_body["data"]["visibility"], "public");

    let authenticated_body = set_custom_config(
        &app,
        &token,
        "custom.authenticated_flag",
        "enabled",
        "authenticated",
    )
    .await;
    assert_eq!(authenticated_body["data"]["visibility"], "authenticated");

    let private_body = set_custom_config(
        &app,
        &token,
        "custom.private_secret",
        "hidden",
        "private",
    )
    .await;
    assert_eq!(private_body["data"]["visibility"], "private");

    let req = test::TestRequest::get()
        .uri("/api/v1/public/custom-config")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("Cache-Control")
            .and_then(|value| value.to_str().ok()),
        Some("public, max-age=60")
    );
    let anonymous_body: Value = test::read_body_json(resp).await;
    assert_eq!(anonymous_body["data"]["entries"]["custom.public_theme"], "nebula");
    assert!(
        anonymous_body["data"]["entries"]
            .get("custom.authenticated_flag")
            .is_none()
    );
    assert!(
        anonymous_body["data"]["entries"]
            .get("custom.private_secret")
            .is_none()
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/public/custom-config")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let authenticated_body: Value = test::read_body_json(resp).await;
    assert_eq!(
        authenticated_body["data"]["entries"]["custom.public_theme"],
        "nebula"
    );
    assert_eq!(
        authenticated_body["data"]["entries"]["custom.authenticated_flag"],
        "enabled"
    );
    assert!(
        authenticated_body["data"]["entries"]
            .get("custom.private_secret")
            .is_none()
    );
}

#[actix_web::test]
async fn test_public_custom_config_rejects_invalid_present_token() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::get()
        .uri("/api/v1/public/custom-config")
        .insert_header(("Authorization", "Bearer fake.token.here"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_system_config_rejects_visibility_update() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/admin/config/branding_title")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(json!({
            "value": "AsterDrive",
            "visibility": "public",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}
