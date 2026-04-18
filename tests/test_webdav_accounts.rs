//! WebDAV 账号管理测试

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_webdav_account_crud() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建 WebDAV 账号
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "webdav_user",
            "password": "webdav_pass123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["username"], "webdav_user");
    let account_id = body["data"]["id"].as_i64().unwrap();

    // 分页列出账号
    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts?limit=1&offset=0")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["limit"], 1);
    assert_eq!(body["data"]["offset"], 0);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);

    // 禁用账号
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}/toggle"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], false);

    // 再次 toggle 启用
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}/toggle"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], true);

    // 删除账号
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 列表应为空
    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 0);
    assert_eq!(body["data"]["total"], 0);
}

#[actix_web::test]
async fn test_webdav_settings_returns_public_endpoint_when_configured() {
    let state = common::setup().await;
    state.runtime_config.apply(common::system_config_model(
        aster_drive::config::site_url::PUBLIC_SITE_URL_KEY,
        "https://drive.example.com",
    ));
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts/settings")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;

    assert_eq!(body["data"]["prefix"], "/webdav");
    assert_eq!(
        body["data"]["endpoint"],
        "https://drive.example.com/webdav/"
    );
}

#[actix_web::test]
async fn test_webdav_settings_falls_back_to_relative_endpoint_without_public_site_url() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts/settings")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;

    assert_eq!(body["data"]["prefix"], "/webdav");
    assert_eq!(body["data"]["endpoint"], "/webdav/");
}

#[actix_web::test]
async fn test_webdav_account_list_resolves_nested_paths() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "A" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let a_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "B", "parent_id": a_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let b_id = body["data"]["id"].as_i64().unwrap();

    for (username, folder_id) in [("path_user_a", a_id), ("path_user_b", b_id)] {
        let req = test::TestRequest::post()
            .uri("/api/v1/webdav-accounts")
            .insert_header(("Cookie", common::access_cookie_header(&token)))
            .insert_header(common::csrf_header_for(&token))
            .set_json(serde_json::json!({
                "username": username,
                "password": "pass123456",
                "root_folder_id": folder_id,
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts?limit=10&offset=0")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let items = body["data"]["items"].as_array().unwrap();
    let path_a = items
        .iter()
        .find(|i| i["username"] == "path_user_a")
        .unwrap()["root_folder_path"]
        .as_str()
        .unwrap();
    let path_b = items
        .iter()
        .find(|i| i["username"] == "path_user_b")
        .unwrap()["root_folder_path"]
        .as_str()
        .unwrap();
    assert_eq!(path_a, "/A");
    assert_eq!(path_b, "/A/B");
}

#[actix_web::test]
async fn test_webdav_account_rejects_duplicate_username_and_foreign_root_folder() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "dup_user",
            "password": "pass123456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "dup_user",
            "password": "pass123456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "other-user",
            "email": "other-user@example.com",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let _ = confirm_latest_contact_verification!(app, db, mail_sender);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "identifier": "other-user",
            "password": "password123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let other_token = common::extract_cookie(&resp, "aster_access").unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&other_token)))
        .insert_header(common::csrf_header_for(&other_token))
        .set_json(serde_json::json!({ "name": "Other Root" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let foreign_folder_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "foreign-root",
            "password": "pass123456",
            "root_folder_id": foreign_folder_id,
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_webdav_account_auto_generated_password_and_disabled_test_rejected() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "auto-pass-user"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let password = body["data"]["password"].as_str().unwrap().to_string();
    assert_eq!(password.len(), 16);
    let account_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "auto-pass-user",
            "password": password
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}/toggle"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "auto-pass-user",
            "password": body["data"]["password"]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_webdav_account_test_connection() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建账号
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "pass1234"
        }))
        .to_request();
    test::call_service(&app, req).await;

    // 测试连接（正确密码）
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "pass1234"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 测试连接（错误密码）
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "wrong"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 401 || resp.status() == 400);
}

#[actix_web::test]
async fn test_webdav_account_rejects_blank_username() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "username": "   ",
            "password": "pass123456"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["msg"], "value cannot be empty");
}
