use actix_web::{App, test, web};
use aster_drive::api;
use aster_drive::runtime::AppState;
use serde_json::Value;

/// 构建一个干净的测试 AppState（内存 SQLite）
async fn setup() -> AppState {
    // 用内存数据库，每次测试隔离
    let db_cfg = aster_drive::config::DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        pool_size: 1,
        retry_count: 0,
    };
    let db = aster_drive::db::connect(&db_cfg).await.unwrap();

    // 跑迁移
    use migration::{Migrator, MigratorTrait};
    Migrator::up(&db, None).await.unwrap();

    // 创建默认本地存储策略
    use chrono::Utc;
    use sea_orm::Set;
    let now = Utc::now();
    let _ = aster_drive::db::repository::policy_repo::create(
        &db,
        aster_drive::entities::storage_policy::ActiveModel {
            name: Set("Test Local".to_string()),
            driver_type: Set(aster_drive::types::DriverType::Local),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set("/tmp/asterdrive-test".to_string()),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set("{}".to_string()),
            is_default: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 清理测试目录
    let _ = std::fs::remove_dir_all("/tmp/asterdrive-test");
    std::fs::create_dir_all("/tmp/asterdrive-test").unwrap();

    AppState {
        db,
        driver_registry: std::sync::Arc::new(aster_drive::storage::DriverRegistry::new()),
        config: std::sync::Arc::new(aster_drive::config::Config {
            auth: aster_drive::config::AuthConfig {
                jwt_secret: "test-secret-key-for-integration-tests".to_string(),
                access_token_ttl_secs: 900,
                refresh_token_ttl_secs: 604800,
            },
            ..Default::default()
        }),
    }
}

/// 从 Set-Cookie header 提取指定 cookie 的值
fn extract_cookie(resp: &actix_web::dev::ServiceResponse, name: &str) -> Option<String> {
    resp.response()
        .cookies()
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())
}

/// 注册 + 登录的宏，返回 (access_cookie, refresh_cookie)
macro_rules! register_and_login {
    ($app:expr) => {{
        // 注册
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": "testuser",
                "email": "test@example.com",
                "password": "password123"
            }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "register should return 201");

        // 登录
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": "testuser",
                "password": "password123"
            }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200, "login should return 200");
        let access = extract_cookie(&resp, "aster_access").expect("access cookie missing");
        let refresh = extract_cookie(&resp, "aster_refresh").expect("refresh cookie missing");
        (access, refresh)
    }};
}

// ─── Tests ───────────────────────────────────────────────

#[actix_web::test]
async fn test_health() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "ok");
}

#[actix_web::test]
async fn test_health_ready() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    let req = test::TestRequest::get().uri("/health/ready").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "ready");
}

#[actix_web::test]
async fn test_register_and_login() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

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
            "username": "alice",
            "password": "secret123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    // tokens 在 cookie 里
    assert!(extract_cookie(&resp, "aster_access").is_some());
    assert!(extract_cookie(&resp, "aster_refresh").is_some());

    // 错误密码
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .set_json(serde_json::json!({
            "username": "alice",
            "password": "wrongpassword"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_token_refresh() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    let (_access, refresh) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .peer_addr("127.0.0.1:12345".parse().unwrap())
        .insert_header(("Cookie", format!("aster_refresh={refresh}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(extract_cookie(&resp, "aster_access").is_some());
}

#[actix_web::test]
async fn test_folders_crud() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    let (token, _) = register_and_login!(app);

    // 列出根目录（应为空）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"], Value::Array(vec![]));
    assert_eq!(body["data"]["files"], Value::Array(vec![]));

    // 创建文件夹
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "Documents" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let folder_id = body["data"]["id"].as_i64().unwrap();
    assert_eq!(body["data"]["name"], "Documents");

    // 列出根目录（应有 1 个文件夹）
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["folders"].as_array().unwrap().len(), 1);

    // 重命名文件夹
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({ "name": "My Docs" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "My Docs");

    // 删除文件夹
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_file_upload_download_delete() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    let (token, _) = register_and_login!(app);

    // 上传文件（multipart）
    let boundary = "----TestBoundary123";
    let file_content = b"Hello AsterDrive!";
    let body = format!(
        "------TestBoundary123\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         {}\r\n\
         ------TestBoundary123--\r\n",
        std::str::from_utf8(file_content).unwrap()
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "upload should return 201 Created");
    let upload_body: Value = test::read_body_json(resp).await;
    assert_eq!(upload_body["code"], 0);
    let file_id = upload_body["data"]["id"].as_i64().unwrap();
    assert_eq!(upload_body["data"]["name"], "hello.txt");
    assert_eq!(upload_body["data"]["mime_type"], "text/plain");

    // 获取文件信息
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["name"], "hello.txt");

    // 下载文件
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}/download"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let download_body = test::read_body(resp).await;
    // multipart 可能带有前导空格，trim 一下
    let content = String::from_utf8_lossy(&download_body);
    assert!(
        content.contains("Hello AsterDrive!"),
        "downloaded content should match: got '{content}'"
    );

    // 列出根目录应该有这个文件
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["files"].as_array().unwrap().len(), 1);

    // 删除文件
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // 再查应该 404
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{file_id}"))
        .insert_header(("Cookie", format!("aster_access={token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_unauthorized_access() {
    let state = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure),
    )
    .await;

    // 没 token 访问受保护端点 — 中间件返回 Error，用 try_call_service
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
