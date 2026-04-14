#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

// ── /me 不泄漏 password_hash 和 config ─────────────────────

#[actix_web::test]
async fn test_me_no_sensitive_fields() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;

    let user = &body["data"]["user"];
    assert!(
        user.get("password_hash").is_none(),
        "password_hash must not be serialized"
    );
    assert!(
        user.get("config").is_none(),
        "config blob must not be serialized"
    );
}

// ── 偏好设置：PATCH 合并 + GET 返回完整值 ───────────────────

#[actix_web::test]
async fn test_preferences_patch_and_get() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 初始状态：无偏好
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(
        body["data"]["preferences"].is_null(),
        "new user should have no preferences"
    );

    // PATCH 设置 theme_mode
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "theme_mode": "dark",
            "browser_open_mode": "double_click",
            "storage_event_stream_enabled": false
        }))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["data"]["theme_mode"], "dark");
    assert_eq!(body["data"]["browser_open_mode"], "double_click");
    assert_eq!(body["data"]["storage_event_stream_enabled"], false);

    // 再 PATCH 设置 language（合并，不覆盖之前的）
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "language": "zh" }))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(
        body["data"]["theme_mode"], "dark",
        "existing pref preserved"
    );
    assert_eq!(body["data"]["browser_open_mode"], "double_click");
    assert_eq!(body["data"]["language"], "zh");
    assert_eq!(body["data"]["storage_event_stream_enabled"], false);

    // /me 也返回完整偏好
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["data"]["preferences"]["theme_mode"], "dark");
    assert_eq!(
        body["data"]["preferences"]["browser_open_mode"],
        "double_click"
    );
    assert_eq!(body["data"]["preferences"]["language"], "zh");
    assert_eq!(
        body["data"]["preferences"]["storage_event_stream_enabled"],
        false
    );
}

// ── 偏好设置：空 PATCH 不改现有值 ──────────────────────────

#[actix_web::test]
async fn test_preferences_empty_patch_noop() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 先设一个值
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "color_preset": "green" }))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["data"]["color_preset"], "green");
    assert!(body["data"]["storage_event_stream_enabled"].is_null());

    // 空 PATCH（全 None）
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({}))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(
        body["data"]["color_preset"], "green",
        "empty patch preserves existing"
    );
    assert!(body["data"]["storage_event_stream_enabled"].is_null());
}

// ── 偏好设置：非法值被拒绝 ────────────────────────────────

#[actix_web::test]
async fn test_preferences_invalid_value_rejected() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "theme_mode": "neon" }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "invalid enum value should be rejected");
}

// ── 未认证访问偏好设置被拒 ────────────────────────────────

#[actix_web::test]
async fn test_preferences_unauthenticated() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .set_json(serde_json::json!({ "theme_mode": "dark" }))
        .to_request();
    // 未认证请求由 JwtAuth 中间件拦截，返回 401
    let result = test::try_call_service(&app, req).await;
    assert!(
        result.is_err(),
        "unauthenticated request should be rejected"
    );
}
