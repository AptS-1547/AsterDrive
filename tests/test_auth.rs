#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;
use std::io::Cursor;

fn avatar_upload_payload() -> (String, Vec<u8>) {
    let boundary = "----AsterAvatarBoundary".to_string();
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        8,
        image::Rgba([255, 120, 0, 255]),
    ));
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"avatar.png\"\r\n\
             Content-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&png.into_inner());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

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
    let access = common::extract_cookie(&resp, "aster_access");
    let refresh = common::extract_cookie(&resp, "aster_refresh");
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);
    // tokens 在 cookie 里
    assert!(access.is_some());
    assert!(refresh.is_some());

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
    let access = common::extract_cookie(&resp, "aster_access");
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);
    assert!(access.is_some());
}

#[actix_web::test]
async fn test_refresh_token_cannot_access_protected_routes() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);
}

#[actix_web::test]
async fn test_logout_clears_cookies_without_revoking_existing_tokens() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header((
            "Cookie",
            format!("aster_access={access}; aster_refresh={refresh}"),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        common::extract_cookie(&resp, "aster_access").as_deref(),
        Some("")
    );
    assert_eq!(
        common::extract_cookie(&resp, "aster_refresh").as_deref(),
        Some("")
    );

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
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
    assert!(body["data"]["access_token_expires_at"].as_i64().unwrap() > 0);
    assert!(body["data"]["password_hash"].is_null());
    assert!(body["data"]["profile"]["display_name"].is_null());
    assert_eq!(body["data"]["profile"]["avatar"]["source"], "none");
}

/// 注册时自动分配新用户默认策略组
#[actix_web::test]
async fn test_register_auto_assigns_policy() {
    use aster_drive::db::repository::policy_group_repo;

    let state = common::setup().await;
    let expected_default_id = policy_group_repo::find_default_group(&state.db)
        .await
        .unwrap()
        .expect("default policy group should exist")
        .id;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 获取用户 ID
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["policy_group_id"].as_i64().unwrap(),
        expected_default_id
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
    assert_service_status!(app, req, 401);
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
        runtime_config: base.runtime_config,
        policy_snapshot: base.policy_snapshot,
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
        runtime_config: base.runtime_config,
        policy_snapshot: base.policy_snapshot,
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
    let bob_refresh = common::extract_cookie(&resp, "aster_refresh").unwrap();

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
    assert_service_status!(app, req, 403, "disabled user should get 403");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={bob_refresh}")))
        .to_request();
    assert_service_status!(app, req, 403, "disabled user refresh should get 403");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/admin/users/{bob_id}"))
        .insert_header(("Cookie", format!("aster_access={admin_token}")))
        .set_json(serde_json::json!({ "status": "active" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={bob_token}")))
        .to_request();
    assert_service_status!(app, req, 401, "old token should stay revoked");

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={bob_refresh}")))
        .to_request();
    assert_service_status!(app, req, 401, "old refresh token should stay revoked");
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

#[actix_web::test]
async fn test_patch_profile_display_name_round_trip_and_clear() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "  Test User  "
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Test User");
    assert_eq!(body["data"]["avatar"]["source"], "none");

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Test User");
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "   "
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["display_name"].is_null());
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["profile"]["display_name"].is_null());
    assert_eq!(body["data"]["profile"]["avatar"]["source"], "upload");
}

#[actix_web::test]
async fn test_change_password_rotates_session_and_updates_login_secret() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, refresh) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/password")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "current_password": "password123",
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let rotated_access = common::extract_cookie(&resp, "aster_access").unwrap();
    let rotated_refresh = common::extract_cookie(&resp, "aster_refresh").unwrap();
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], 0);
    assert_eq!(body["data"]["expires_in"], 900);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={rotated_access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    assert_service_status!(app, req, 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Cookie", format!("aster_refresh={rotated_refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_change_password_rejects_wrong_current_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/password")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "current_password": "wrongpassword",
            "new_password": "newsecret456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "testuser",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_patch_profile_rejects_overlong_display_name() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "a".repeat(65)
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["data"]["profile"]["display_name"].is_null());
}

#[actix_web::test]
async fn test_display_name_survives_avatar_source_switches() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::patch()
        .uri("/api/v1/auth/profile")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "display_name": "Avatar User"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["display_name"], "Avatar User");
    assert_eq!(body["data"]["avatar"]["source"], "upload");

    for source in ["gravatar", "none"] {
        let req = test::TestRequest::put()
            .uri("/api/v1/auth/profile/avatar/source")
            .insert_header(("Cookie", format!("aster_access={token}")))
            .set_json(serde_json::json!({ "source": source }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["display_name"], "Avatar User");
        assert_eq!(body["data"]["avatar"]["source"], source);
    }
}

#[actix_web::test]
async fn test_avatar_upload_and_source_switch() {
    let state = common::setup().await;
    let avatar_base_path = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .base_path;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "upload");
    assert_eq!(body["data"]["avatar"]["version"], 1);
    assert_eq!(
        body["data"]["avatar"]["url_512"],
        "/auth/profile/avatar/512?v=1"
    );
    let avatar_v1_512 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/512.webp"));
    let avatar_v1_1024 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/1024.webp"));
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/webp");

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/profile/avatar/source")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "source": "gravatar"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "gravatar");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert!(
        body["data"]["avatar"]["url_512"]
            .as_str()
            .unwrap()
            .contains("gravatar.com/avatar/")
    );
    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_avatar_reupload_replaces_previous_objects() {
    let state = common::setup().await;
    let avatar_base_path = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .base_path;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_v1_512 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/512.webp"));
    let avatar_v1_1024 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/1024.webp"));
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "upload");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert_eq!(
        body["data"]["avatar"]["url_512"],
        "/auth/profile/avatar/512?v=2"
    );

    let avatar_v2_512 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v2/512.webp"));
    let avatar_v2_1024 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v2/1024.webp"));
    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());
    assert!(avatar_v2_512.exists());
    assert!(avatar_v2_1024.exists());
}

#[actix_web::test]
async fn test_avatar_switch_to_none_deletes_uploaded_objects() {
    let state = common::setup().await;
    let avatar_base_path = aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist")
        .base_path;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let user_id = body["data"]["id"].as_i64().unwrap();

    let (boundary, payload) = avatar_upload_payload();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/profile/avatar/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let avatar_v1_512 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/512.webp"));
    let avatar_v1_1024 = std::path::PathBuf::from(&avatar_base_path)
        .join(format!("profile/avatar/{user_id}/v1/1024.webp"));
    assert!(avatar_v1_512.exists());
    assert!(avatar_v1_1024.exists());

    let req = test::TestRequest::put()
        .uri("/api/v1/auth/profile/avatar/source")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "source": "none"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["avatar"]["source"], "none");
    assert_eq!(body["data"]["avatar"]["version"], 2);
    assert!(body["data"]["avatar"]["url_512"].is_null());

    assert!(!avatar_v1_512.exists());
    assert!(!avatar_v1_1024.exists());

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/profile/avatar/512")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
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
