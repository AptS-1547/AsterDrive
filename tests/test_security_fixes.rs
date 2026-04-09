//! 安全修复回归测试
//! - Fix 1: 上传不能越权到别人的文件夹
//! - Fix 2: update_storage_used 减量不下溢
//! - Fix 3: 分享下载 304 不应增加 download_count

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;

// ─── Fix 1: 越权上传被拒 ───────────────────────────────────

/// 注册第二个用户并登录，返回 access_token
macro_rules! register_user2 {
    ($app:expr, $mail_sender:expr) => {{
        use actix_web::test;

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": "user2",
                "email": "user2@example.com",
                "password": "password123"
            }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);

        let _ = confirm_latest_contact_verification!($app, $mail_sender);

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "identifier": "user2",
                "password": "password123"
            }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        common::extract_cookie(&resp, "aster_access").unwrap()
    }};
}

#[actix_web::test]
async fn test_upload_to_other_users_folder_returns_403() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token1, _) = register_and_login!(app);
    let token2 = register_user2!(app, mail_sender);

    // user1 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token1}")))
        .set_json(serde_json::json!({ "name": "private" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // user2 尝试上传到 user1 的文件夹 → 403
    let boundary = "----CrossUserBoundary";
    let payload = "------CrossUserBoundary\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"evil.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         pwned\r\n\
         ------CrossUserBoundary--\r\n"
        .to_string();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/upload?folder_id={folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token2}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "uploading to another user's folder should return 403"
    );
}

#[actix_web::test]
async fn test_init_upload_to_other_users_folder_returns_403() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token1, _) = register_and_login!(app);
    let token2 = register_user2!(app, mail_sender);

    // user1 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token1}")))
        .set_json(serde_json::json!({ "name": "secret" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // user2 尝试 init_upload 到 user1 的文件夹 → 403
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token2}")))
        .set_json(serde_json::json!({
            "filename": "evil.bin",
            "total_size": 1024,
            "folder_id": folder_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "init_upload to another user's folder should return 403"
    );
}

#[actix_web::test]
async fn test_directory_upload_to_other_users_base_folder_returns_403() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token1, _) = register_and_login!(app);
    let token2 = register_user2!(app, mail_sender);

    // user1 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token1}")))
        .set_json(serde_json::json!({ "name": "base" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // user2 尝试目录上传到 user1 的文件夹 → 403
    let boundary = "----DirCrossBoundary";
    let payload = "------DirCrossBoundary\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"sneaky.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         pwned via directory upload\r\n\
         ------DirCrossBoundary--\r\n"
        .to_string();
    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/files/upload?folder_id={folder_id}&relative_path=sub/sneaky.txt"
        ))
        .insert_header(("Cookie", format!("aster_access={token2}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "directory upload to another user's base folder should return 403"
    );
}

// ─── A1: 在别人文件夹下建子文件夹被拒 ──────────────────────

#[actix_web::test]
async fn test_create_folder_in_other_users_folder_returns_403() {
    let state = common::setup().await;
    let mail_sender = state.mail_sender.clone();
    let app = create_test_app!(state);
    let (token1, _) = register_and_login!(app);
    let token2 = register_user2!(app, mail_sender);

    // user1 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token1}")))
        .set_json(serde_json::json!({ "name": "user1-only" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();

    // user2 尝试在 user1 的文件夹下建子文件夹 → 403
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token2}")))
        .set_json(serde_json::json!({
            "name": "evil-subfolder",
            "parent_id": folder_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "creating folder in another user's folder should return 403"
    );
}

// ─── Fix 3: 分享下载 304 不应计数 ──────────────────────────

#[actix_web::test]
async fn test_share_download_304_does_not_increment_count() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    // 创建不限次数的分享
    let req = test::TestRequest::post()
        .uri("/api/v1/shares")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "file_id": file_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let share_token = body["data"]["token"].as_str().unwrap().to_string();

    // 第一次下载 → 200，拿 ETag
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let etag = resp
        .headers()
        .get("ETag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 查 download_count = 1
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["download_count"], 1,
        "first download should count"
    );

    // 带 If-None-Match 再次请求 → 304
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}/download"))
        .insert_header(("If-None-Match", etag.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 304, "should return 304 for matching ETag");

    // download_count 应该仍然是 1
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/s/{share_token}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["download_count"], 1,
        "304 cache hit should NOT increment download_count"
    );
}
