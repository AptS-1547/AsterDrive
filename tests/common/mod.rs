use aster_drive::runtime::AppState;

/// 构建一个干净的测试 AppState（内存 SQLite）
pub async fn setup() -> AppState {
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

    // 每个测试用独立临时目录避免并行竞争
    let test_dir = format!("/tmp/asterdrive-test-{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&test_dir).unwrap();

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
            base_path: Set(test_dir),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set("{}".to_string()),
            is_default: Set(true),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 测试用 NoopCache
    let cache_config = aster_drive::config::CacheConfig {
        enabled: false,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    let config = std::sync::Arc::new(aster_drive::config::Config {
        auth: aster_drive::config::AuthConfig {
            jwt_secret: "test-secret-key-for-integration-tests".to_string(),
            access_token_ttl_secs: 900,
            refresh_token_ttl_secs: 604800,
        },
        ..Default::default()
    });

    // 初始化全局 config（WebDAV file.rs 内部调 get_config() 需要）
    // OnceLock 只设置一次，后续调用忽略
    let _ = aster_drive::config::set_config_for_test(config.clone());

    AppState {
        db,
        driver_registry: std::sync::Arc::new(aster_drive::storage::DriverRegistry::new()),
        config,
        cache,
    }
}

/// 从 Set-Cookie header 提取指定 cookie 的值
#[allow(dead_code)]
pub fn extract_cookie(resp: &actix_web::dev::ServiceResponse, name: &str) -> Option<String> {
    resp.response()
        .cookies()
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())
}

/// 创建标准测试 App
#[macro_export]
macro_rules! create_test_app {
    ($state:expr) => {{
        use actix_web::{App, test, web};

        let state = $state;
        let db = state.db.clone();
        test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
        )
        .await
    }};
}

/// 注册 + 登录，返回 (access_cookie, refresh_cookie)
#[macro_export]
macro_rules! register_and_login {
    ($app:expr) => {{
        use actix_web::test;

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
                "identifier": "testuser",
                "password": "password123"
            }))
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200, "login should return 200");
        let access =
            common::extract_cookie(&resp, "aster_access").expect("access cookie missing");
        let refresh =
            common::extract_cookie(&resp, "aster_refresh").expect("refresh cookie missing");
        (access, refresh)
    }};
}

/// 上传测试文件，返回 file_id
#[macro_export]
macro_rules! upload_test_file {
    ($app:expr, $token:expr) => {{
        use actix_web::test;
        use serde_json::Value;

        let boundary = "----TestBoundary123";
        let payload = format!(
            "------TestBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             test content\r\n\
             ------TestBoundary123--\r\n"
        );
        let req = test::TestRequest::post()
            .uri("/api/v1/files/upload")
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "upload should return 201");
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

/// 上传测试文件到指定文件夹，返回 file_id
#[macro_export]
macro_rules! upload_test_file_to_folder {
    ($app:expr, $token:expr, $folder_id:expr) => {{
        use actix_web::test;
        use serde_json::Value;

        let boundary = "----TestBoundary123";
        let payload = format!(
            "------TestBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"test-in-folder.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             test content in folder\r\n\
             ------TestBoundary123--\r\n"
        );
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload?folder_id={}", $folder_id))
            .insert_header(("Cookie", format!("aster_access={}", $token)))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp: actix_web::dev::ServiceResponse = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "upload to folder should return 201");
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

/// 构建带 WebDAV 路由的测试 App
#[macro_export]
macro_rules! setup_with_webdav {
    () => {{
        use actix_web::{App, test, web};

        let state = common::setup().await;
        let db1 = state.db.clone();
        let db2 = state.db.clone();
        let webdav_config = aster_drive::config::WebDavConfig::default();
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(
            move |cfg| {
                aster_drive::webdav::configure(cfg, &webdav_config, &db2);
                aster_drive::api::configure(cfg, &db1);
            },
        ))
        .await;
        app
    }};
}
