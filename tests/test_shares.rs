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
    assert_eq!(body["data"]["mime_type"], "text/plain");
    assert!(body["data"]["size"].as_i64().unwrap() > 0);

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

#[actix_web::test]
async fn test_share_download_limit() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 创建限 1 次下载的分享
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "file_id": file_id,
            "max_downloads": 1
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();

    // 第一次下载 OK
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 第二次下载应被拒绝（403 或 410）
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status() == 403 || resp.status() == 410,
        "download limit should block, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn test_share_folder() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Shared Folder" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // 上传一个文件到该文件夹
    let file_id = upload_test_file_to_folder!(app, token, folder_id);

    // 分享文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "folder_id": folder_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();

    // 公开查看分享信息
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["share_type"], "folder");

    // 公开列出文件夹内容
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/content"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 下载文件夹内文件
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/files/{file_id}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

/// 伪造 cookie 不能绕过密码验证
#[actix_web::test]
async fn test_share_forged_cookie_rejected() {
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
            "password": "secret"
        }))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();

    // 用伪造 cookie 尝试下载 → 应被拒绝
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .insert_header(("Cookie", format!("aster_share_{share_token}=forged_value")))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "forged cookie should be rejected, got {}",
        resp.status()
    );

    // 用正确流程验证密码
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/s/{share_token}/verify"))
        .set_json(serde_json::json!({"password": "secret"}))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 提取签名 cookie
    let signed_cookie = common::extract_cookie(&resp, &format!("aster_share_{share_token}"))
        .expect("should get signed cookie");

    // 用签名 cookie 下载 → 应成功
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .insert_header((
            "Cookie",
            format!("aster_share_{share_token}={signed_cookie}"),
        ))
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "signed cookie should allow download, got {}",
        resp.status()
    );
}
