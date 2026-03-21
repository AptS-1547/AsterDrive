#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

#[actix_web::test]
async fn test_shares_crud() {
    let state = common::setup().await;
    let app = create_test_app!(state);

    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 创建分享
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "file_id": file_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();
    let share_id = body["data"]["id"].as_i64().unwrap();

    // 列出分享
    let req = test::TestRequest::get()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // 公开访问分享信息
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "test.txt");

    // 公开下载
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 删除分享
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/shares/{share_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 分享不再可访问
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 404 || resp.status() == 410);
}

#[actix_web::test]
async fn test_share_password() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 创建带密码分享
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "file_id": file_id,
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();

    // 公开访问 — 应显示 has_password=true
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["has_password"], true);

    // 无密码下载 — 应被拦截（403）
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);

    // 验证密码
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/s/{share_token}/verify"))
        .set_json(serde_json::json!({ "password": "secret123" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 错误密码
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/s/{share_token}/verify"))
        .set_json(serde_json::json!({ "password": "wrong" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 401 || resp.status() == 403);
}
