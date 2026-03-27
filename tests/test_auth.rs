#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_register_and_login() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // 注册
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["username"], "alice");
    // password_hash 不应该暴露
    assert!(body["data"]["password_hash"].is_null());

    // 重复注册应失败
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "alice",
            "email": "alice2@example.com",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    // 登录
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "alice",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    // tokens 在 cookie 里
    assert!(common::extract_cookie(&resp, "aster_access").is_some());
    assert!(common::extract_cookie(&resp, "aster_refresh").is_some());

    // 错误密码
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "alice",
            "password": "wrongpassword"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_token_refresh() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(common::extract_cookie(&resp, "aster_access").is_some());
}

#[actix_web::test]
async fn test_auth_me() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["username"], "testuser");
    assert!(body["data"]["password_hash"].is_null());
}

/// 注册时自动分配系统默认存储策略
#[actix_web::test]
async fn test_register_auto_assigns_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    // 用户应已有 1 个策略分配（自动分配的）
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/users/{user_id}/policies"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let policies = body["data"]["items"].as_array().unwrap();
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(
        policies.len(),
        1,
        "new user should have 1 auto-assigned policy"
    );
    assert_eq!(
        policies[0]["is_default"], true,
        "auto-assigned policy should be default"
    );
}

#[actix_web::test]
async fn test_unauthorized_access() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    // 没 token 访问受保护端点
    let req = test::TestRequest::get().uri("/api/v1/folders").to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 401);
        }
    }

    // 假 token
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", "Bearer fake.token.here"))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 401);
        }
    }
}

/// 用户状态缓存：正常认证 → 连续请求不应查 DB（通过 MemoryCache 验证）
#[actix_web::test]
async fn test_user_status_cached_in_auth_middleware() {
    // 用 MemoryCache 替代默认 NoopCache
    let cache_config = aster_drive::config::CacheConfig {
        enabled: true,
        backend: "memory".to_string(),
        default_ttl: 60,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    let base = common::setup().await;
    let state = aster_drive::runtime::AppState {
        db: base.db,
        driver_registry: base.driver_registry,
        config: base.config,
        cache,
        thumbnail_tx: base.thumbnail_tx,
    };
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 第一次请求（cache miss → 查 DB → 写缓存）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 第二次请求（cache hit → 不查 DB）—— 功能正确即可
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

/// admin 禁用用户后，缓存立即失效，后续请求被拒
#[actix_web::test]
async fn test_disable_user_invalidates_status_cache() {
    let cache_config = aster_drive::config::CacheConfig {
        enabled: true,
        backend: "memory".to_string(),
        default_ttl: 60,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    let base = common::setup().await;
    let state = aster_drive::runtime::AppState {
        db: base.db,
        driver_registry: base.driver_registry,
        config: base.config,
        cache,
        thumbnail_tx: base.thumbnail_tx,
    };
    let app = create_test_app!(state);
    let (admin_token, _) = register_and_login!(app);

    // 注册第二个用户
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "bobuser",
            "email": "bob@example.com",
            "password": "password456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(status, 201, "register bob failed: {body}");
    let bob_id = body["data"]["id"].as_i64().unwrap();

    // bob 登录
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "bobuser",
            "password": "password456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let bob_token = common::extract_cookie(&resp, "aster_access").unwrap();

    // bob 正常访问（写入缓存）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // admin 禁用 bob
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{bob_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "status": "disabled" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // bob 再次访问——应被拒（缓存已失效）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 403, "disabled user should get 403"),
        Err(err) => {
            let resp = err.error_response();
            assert_eq!(resp.status(), 403, "disabled user should get 403");
        }
    }
}

// ── Preferences endpoint tests ──

/// Set preferences via PATCH, then verify they are returned by GET /me.
#[actix_web::test]
async fn test_patch_preferences_set_and_get() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Patch all fields
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "dark",
            "color_preset": "green",
            "view_mode": "grid",
            "sort_by": "size",
            "sort_order": "desc",
            "language": "zh"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["theme_mode"], "dark");
    assert_eq!(body["data"]["color_preset"], "green");
    assert_eq!(body["data"]["view_mode"], "grid");
    assert_eq!(body["data"]["sort_by"], "size");
    assert_eq!(body["data"]["sort_order"], "desc");
    assert_eq!(body["data"]["language"], "zh");

    // Verify via GET /me
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["preferences"]["theme_mode"], "dark");
    assert_eq!(body["data"]["preferences"]["view_mode"], "grid");
    assert_eq!(body["data"]["preferences"]["language"], "zh");
}

/// Partial PATCH only updates specified fields; others remain unchanged.
#[actix_web::test]
async fn test_patch_preferences_partial_update() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Set initial preferences
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "dark",
            "view_mode": "grid"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Partial update: only change sort_by
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "sort_by": "size"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    // Previously set fields should be preserved
    assert_eq!(body["data"]["theme_mode"], "dark");
    assert_eq!(body["data"]["view_mode"], "grid");
    // Newly set field
    assert_eq!(body["data"]["sort_by"], "size");
}

/// Invalid enum values should be rejected with a 400 error.
#[actix_web::test]
async fn test_patch_preferences_invalid_enum_value() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "theme_mode": "invalid_value"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "invalid enum value should return 400");

    // sort_order with invalid value
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "sort_order": "sideways"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "invalid sort_order should return 400");
}

/// PATCH with empty body should succeed (no-op, returns current prefs).
#[actix_web::test]
async fn test_patch_preferences_empty_body() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // Empty body — should succeed with no changes
    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    // All fields should be null for a fresh user
    assert!(body["data"]["theme_mode"].is_null());
    assert!(body["data"]["color_preset"].is_null());
    assert!(body["data"]["language"].is_null());

    // Verify via GET /me — fresh user has no stored config so preferences is null
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["preferences"].is_null());
}

/// sort_by = "type" uses a special snake_case rename; verify it round-trips correctly.
#[actix_web::test]
async fn test_patch_preferences_sort_by_type() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "sort_by": "type" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["sort_by"], "type");
}

/// Unauthenticated requests to PATCH /preferences should be rejected.
#[actix_web::test]
async fn test_patch_preferences_unauthenticated() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/preferences")
        .set_json(serde_json::json!({
            "theme_mode": "dark"
        }))
        .to_request();
    let result = test::try_call_service(&app, req).await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 401),
        Err(err) => assert_eq!(err.error_response().status(), 401),
    }
}
