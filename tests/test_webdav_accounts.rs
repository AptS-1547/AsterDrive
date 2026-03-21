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
        .insert_header(("Cookie", format!("aster_access={token}")))
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

    // 列出账号
    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // 禁用账号
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}/toggle"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], false);

    // 再次 toggle 启用
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}/toggle"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], true);

    // 删除账号
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/webdav-accounts/{account_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 列表应为空
    let req = test::TestRequest::get()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_webdav_account_test_connection() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建账号
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "pass123"
        }))
        .to_request();
    test::call_service(&app, req).await;

    // 测试连接（正确密码）
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "pass123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 测试连接（错误密码）
    let req = test::TestRequest::post()
        .uri("/api/v1/webdav-accounts/test")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "username": "test_conn",
            "password": "wrong"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 401 || resp.status() == 400);
}
