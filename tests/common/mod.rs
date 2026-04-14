use aster_drive::runtime::AppState;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

thread_local! {
    static CSRF_LOOKUP_CACHE: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn csrf_registry() -> &'static Mutex<HashMap<String, String>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_csrf_registry() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    csrf_registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[allow(dead_code)]
pub fn remember_csrf_token(session_token: &str, csrf_token: &str) {
    if session_token.is_empty() || csrf_token.is_empty() {
        return;
    }

    CSRF_LOOKUP_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(session_token.to_string(), csrf_token.to_string());
    });

    lock_csrf_registry().insert(session_token.to_string(), csrf_token.to_string());
}

#[allow(dead_code)]
pub fn seed_csrf_token(session_token: &str) -> String {
    let csrf_token = aster_drive::api::middleware::csrf::build_csrf_token();
    remember_csrf_token(session_token, &csrf_token);
    csrf_token
}

#[allow(dead_code)]
pub fn csrf_token_for(session_token: impl AsRef<str>) -> String {
    let session_token = session_token.as_ref();
    if let Some(token) = CSRF_LOOKUP_CACHE.with(|cache| cache.borrow().get(session_token).cloned())
    {
        return token;
    }

    lock_csrf_registry()
        .get(session_token)
        .cloned()
        .map(|csrf_token| {
            CSRF_LOOKUP_CACHE.with(|cache| {
                cache
                    .borrow_mut()
                    .insert(session_token.to_string(), csrf_token.clone());
            });
            csrf_token
        })
        .unwrap_or_else(|| panic!("missing csrf token for session token: {session_token}"))
}

#[allow(dead_code)]
pub fn access_cookie_header(access_token: impl AsRef<str>) -> String {
    let access_token = access_token.as_ref();
    format!(
        "aster_access={access_token}; aster_csrf={}",
        csrf_token_for(access_token)
    )
}

#[allow(dead_code)]
pub fn refresh_cookie_header(refresh_token: impl AsRef<str>) -> String {
    let refresh_token = refresh_token.as_ref();
    format!(
        "aster_refresh={refresh_token}; aster_csrf={}",
        csrf_token_for(refresh_token)
    )
}

#[allow(dead_code)]
pub fn access_and_refresh_cookie_header(
    access_token: impl AsRef<str>,
    refresh_token: impl AsRef<str>,
) -> String {
    let access_token = access_token.as_ref();
    let refresh_token = refresh_token.as_ref();
    format!(
        "aster_access={access_token}; aster_refresh={refresh_token}; aster_csrf={}",
        csrf_token_for(access_token)
    )
}

#[allow(dead_code)]
pub fn csrf_header_for(session_token: impl AsRef<str>) -> (&'static str, String) {
    ("X-CSRF-Token", csrf_token_for(session_token))
}

/// 构建一个干净的测试 AppState（内存 SQLite）
#[allow(dead_code)]
pub async fn setup() -> AppState {
    setup_with_database_url("sqlite::memory:").await
}

/// 构建一个干净的测试 AppState（指定数据库 URL）
pub async fn setup_with_database_url(database_url: &str) -> AppState {
    let db_cfg = aster_drive::config::DatabaseConfig {
        url: database_url.to_string(),
        pool_size: 1,
        retry_count: 0,
    };
    let db = aster_drive::db::connect(&db_cfg).await.unwrap();

    // 跑迁移
    use migration::{Migrator, MigratorTrait};
    Migrator::up(&db, None).await.unwrap();

    // 每个测试用独立临时目录避免并行竞争
    let test_dir = format!("/tmp/asterdrive-test-{}", uuid::Uuid::new_v4());
    let temp_dir = format!("{test_dir}/temp");
    let upload_temp_dir = format!("{test_dir}/uploads");
    let avatar_dir = format!("{test_dir}/avatar");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::create_dir_all(&upload_temp_dir).unwrap();
    std::fs::create_dir_all(&avatar_dir).unwrap();

    let config = std::sync::Arc::new(aster_drive::config::Config {
        server: aster_drive::config::ServerConfig {
            temp_dir,
            upload_temp_dir,
            ..Default::default()
        },
        auth: aster_drive::config::AuthConfig {
            jwt_secret: "test-secret-key-for-integration-tests".to_string(),
            bootstrap_insecure_cookies: true,
        },
        ..Default::default()
    });

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

    aster_drive::services::policy_service::ensure_policy_groups_seeded(&db)
        .await
        .unwrap();

    aster_drive::db::repository::config_repo::ensure_system_value_if_missing(
        &db,
        aster_drive::config::auth_runtime::AUTH_COOKIE_SECURE_KEY,
        "false",
    )
    .await
    .unwrap();

    aster_drive::db::repository::config_repo::ensure_defaults(&db)
        .await
        .unwrap();
    aster_drive::db::repository::config_repo::upsert(
        &db,
        aster_drive::config::avatar::AVATAR_DIR_KEY,
        &avatar_dir,
        0,
    )
    .await
    .unwrap();

    // 测试用 NoopCache
    let cache_config = aster_drive::config::CacheConfig {
        enabled: false,
        ..Default::default()
    };
    let cache = aster_drive::cache::create_cache(&cache_config).await;

    // 初始化全局 config（WebDAV file.rs 内部调 get_config() 需要）
    // OnceLock 只设置一次，后续调用忽略
    let _ = aster_drive::config::set_config_for_test(config.clone());

    let runtime_config = std::sync::Arc::new(aster_drive::config::RuntimeConfig::new());
    runtime_config.reload(&db).await.unwrap();

    let policy_snapshot = std::sync::Arc::new(aster_drive::storage::PolicySnapshot::new());
    policy_snapshot.reload(&db).await.unwrap();
    let mail_sender = aster_drive::services::mail_service::memory_sender();

    let (thumbnail_tx, _thumbnail_rx) = tokio::sync::mpsc::channel::<i64>(16);
    let (storage_change_tx, _) = tokio::sync::broadcast::channel(
        aster_drive::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
    );

    AppState {
        db,
        driver_registry: std::sync::Arc::new(aster_drive::storage::DriverRegistry::new()),
        runtime_config,
        policy_snapshot,
        config,
        cache,
        mail_sender,
        thumbnail_tx,
        storage_change_tx,
    }
}

#[allow(dead_code)]
pub async fn flush_mail_outbox(state: &AppState) {
    flush_mail_outbox_with(&state.db, &state.runtime_config, &state.mail_sender).await;
}

#[allow(dead_code)]
pub async fn flush_mail_outbox_with(
    db: &sea_orm::DatabaseConnection,
    runtime_config: &std::sync::Arc<aster_drive::config::RuntimeConfig>,
    mail_sender: &std::sync::Arc<dyn aster_drive::services::mail_service::MailSender>,
) {
    aster_drive::services::mail_outbox_service::drain_with(db, runtime_config, mail_sender)
        .await
        .expect("mail outbox drain should succeed");
}

/// 从 Set-Cookie header 提取指定 cookie 的值
#[allow(dead_code)]
pub fn extract_cookie<B>(resp: &actix_web::dev::ServiceResponse<B>, name: &str) -> Option<String> {
    let value = resp
        .response()
        .cookies()
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())?;

    if matches!(name, "aster_access" | "aster_refresh") {
        if let Some(csrf_token) = resp
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "aster_csrf")
            .map(|cookie| cookie.value().to_string())
        {
            remember_csrf_token(&value, &csrf_token);
        }
    }

    Some(value)
}

#[allow(dead_code)]
fn extract_token_from_content(content: &str, marker: &str) -> Option<String> {
    let (_, suffix) = content.split_once(marker)?;
    let encoded: String = suffix
        .chars()
        .take_while(|ch| !matches!(ch, '"' | '\'' | '<' | '>' | '&' | ' ' | '\r' | '\n'))
        .collect();
    if encoded.is_empty() {
        return None;
    }

    urlencoding::decode(&encoded)
        .ok()
        .map(|value| value.into_owned())
}

#[allow(dead_code)]
pub fn extract_token_from_mail_message(
    message: &aster_drive::services::mail_service::MailMessage,
    marker: &str,
) -> Option<String> {
    extract_token_from_content(&message.text_body, marker)
        .or_else(|| extract_token_from_content(&message.html_body, marker))
}

#[allow(dead_code)]
pub fn extract_verification_token_from_mail_sender(
    sender: &Arc<dyn aster_drive::services::mail_service::MailSender>,
) -> Option<String> {
    let memory_sender = aster_drive::services::mail_service::memory_sender_ref(sender)
        .expect("memory mail sender should be available in tests");
    let message = memory_sender.last_message()?;
    extract_token_from_mail_message(&message, "/api/v1/auth/contact-verification/confirm?token=")
}

#[allow(dead_code)]
pub async fn extract_verification_token_from_mail_sender_or_outbox(
    db: &sea_orm::DatabaseConnection,
    sender: &Arc<dyn aster_drive::services::mail_service::MailSender>,
) -> Option<String> {
    if let Some(token) = extract_verification_token_from_mail_sender(sender) {
        return Some(token);
    }

    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let row = aster_drive::entities::mail_outbox::Entity::find()
        .filter(
            aster_drive::entities::mail_outbox::Column::TemplateCode.is_in([
                aster_drive::types::MailTemplateCode::RegisterActivation,
                aster_drive::types::MailTemplateCode::ContactChangeConfirmation,
            ]),
        )
        .order_by_desc(aster_drive::entities::mail_outbox::Column::Id)
        .one(db)
        .await
        .expect("mail outbox lookup should succeed")?;

    serde_json::from_str::<serde_json::Value>(&row.payload_json)
        .expect("mail outbox payload should be valid json")
        .get("token")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

#[allow(dead_code)]
pub fn system_config_model(key: &str, value: &str) -> aster_drive::entities::system_config::Model {
    aster_drive::entities::system_config::Model {
        id: 0,
        key: key.to_string(),
        value: value.to_string(),
        value_type: "string".to_string(),
        requires_restart: false,
        is_sensitive: false,
        source: "system".to_string(),
        namespace: String::new(),
        category: "test".to_string(),
        description: "test".to_string(),
        updated_at: chrono::Utc::now(),
        updated_by: None,
    }
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
                .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
                .app_data(web::JsonConfig::default().limit(1024 * 1024))
                .app_data(web::Data::new(state))
                .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
        )
        .await
    }};
}

/// 兼容 `call_service` / `try_call_service` 两种返回路径的状态断言
#[macro_export]
macro_rules! assert_service_status {
    ($app:expr, $req:expr, $status:expr) => {{
        use actix_web::test;

        let result = test::try_call_service(&$app, $req).await;
        match result {
            Ok(resp) => assert_eq!(resp.status(), $status),
            Err(err) => {
                let resp = err.error_response();
                assert_eq!(resp.status(), $status);
            }
        }
    }};
    ($app:expr, $req:expr, $status:expr, $msg:expr) => {{
        use actix_web::test;

        let result = test::try_call_service(&$app, $req).await;
        match result {
            Ok(resp) => assert_eq!(resp.status(), $status, $msg),
            Err(err) => {
                let resp = err.error_response();
                assert_eq!(resp.status(), $status, $msg);
            }
        }
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
        let resp = test::call_service(&$app, req).await;
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
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200, "login should return 200");
        let access =
            common::extract_cookie(&resp, "aster_access").expect("access cookie missing");
        let refresh =
            common::extract_cookie(&resp, "aster_refresh").expect("refresh cookie missing");
        (access, refresh)
    }};
}

/// 管理员创建普通用户，返回 user_id
#[macro_export]
macro_rules! admin_create_user {
    ($app:expr, $admin_token:expr, $username:expr, $email:expr, $password:expr) => {{
        use actix_web::test;
        use serde_json::Value;

        let req = test::TestRequest::post()
            .uri("/api/v1/admin/users")
            .insert_header(("Cookie", common::access_cookie_header(&$admin_token)))
            .insert_header(common::csrf_header_for(&$admin_token))
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": $username,
                "email": $email,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "admin create user should return 201");
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

/// 使用用户名/邮箱登录，返回 (access_cookie, refresh_cookie)
#[macro_export]
macro_rules! login_user {
    ($app:expr, $identifier:expr, $password:expr) => {{
        use actix_web::test;

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "identifier": $identifier,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200, "login should return 200");
        let access =
            common::extract_cookie(&resp, "aster_access").expect("access cookie missing");
        let refresh =
            common::extract_cookie(&resp, "aster_refresh").expect("refresh cookie missing");
        (access, refresh)
    }};
}

#[macro_export]
macro_rules! confirm_latest_contact_verification {
    ($app:expr, $db:expr, $mail_sender:expr) => {{
        use actix_web::test;

        if let Some(token) =
            common::extract_verification_token_from_mail_sender_or_outbox(&$db, &$mail_sender).await
        {
            let req = test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/auth/contact-verification/confirm?token={}",
                    urlencoding::encode(&token)
                ))
                .to_request();
            let resp = test::call_service(&$app, req).await;
            assert_eq!(resp.status(), 302, "contact verification should return 302");
            let location = resp
                .headers()
                .get("Location")
                .and_then(|value| value.to_str().ok())
                .expect("contact verification redirect location missing")
                .to_string();
            Some(location)
        } else {
            None
        }
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
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201, "upload should return 201");
        let body: Value = test::read_body_json(resp).await;
        body["data"]["id"].as_i64().unwrap()
    }};
}

/// 上传指定名称测试文件，返回 file_id
#[macro_export]
macro_rules! upload_test_file_named {
    ($app:expr, $token:expr, $name:expr) => {{
        use actix_web::test;
        use serde_json::Value;

        let boundary = "----TestBoundary123";
        let payload = format!(
            "------TestBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             test content\r\n\
             ------TestBoundary123--\r\n",
            name = $name
        );
        let req = test::TestRequest::post()
            .uri("/api/v1/files/upload")
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&$app, req).await;
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
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request();
        let resp = test::call_service(&$app, req).await;
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
        let app = test::init_service(
            App::new()
                .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
                .app_data(web::JsonConfig::default().limit(1024 * 1024))
                .app_data(web::Data::new(state))
                .configure(move |cfg| {
                    aster_drive::webdav::configure(cfg, &webdav_config, &db2);
                    aster_drive::api::configure(cfg, &db1);
                }),
        )
        .await;
        app
    }};
}

#[macro_export]
macro_rules! setup_with_webdav_and_mail {
    () => {{
        use actix_web::{App, test, web};

        let state = common::setup().await;
        let db = state.db.clone();
        let mail_sender = state.mail_sender.clone();
        let db1 = state.db.clone();
        let db2 = state.db.clone();
        let webdav_config = aster_drive::config::WebDavConfig::default();
        let app = test::init_service(
            App::new()
                .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
                .app_data(web::JsonConfig::default().limit(1024 * 1024))
                .app_data(web::Data::new(state))
                .configure(move |cfg| {
                    aster_drive::webdav::configure(cfg, &webdav_config, &db2);
                    aster_drive::api::configure(cfg, &db1);
                }),
        )
        .await;
        (app, db, mail_sender)
    }};
}
