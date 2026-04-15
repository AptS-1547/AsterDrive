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

const TEST_DATABASE_BACKEND_ENV: &str = "ASTER_TEST_DATABASE_BACKEND";
// Keep the year within MySQL TIMESTAMP's supported range.
#[allow(dead_code)]
pub const TEST_FUTURE_SHARE_EXPIRY_RFC3339: &str = "2037-04-02T12:00:00Z";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestDatabaseBackend {
    Sqlite,
    Postgres,
    MySql,
}

struct SharedTestDatabaseContainer {
    _container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
    admin_database_url: String,
    database_url: String,
}

static POSTGRES_TEST_CONTAINER: tokio::sync::OnceCell<SharedTestDatabaseContainer> =
    tokio::sync::OnceCell::const_new();
static MYSQL_TEST_CONTAINER: tokio::sync::OnceCell<SharedTestDatabaseContainer> =
    tokio::sync::OnceCell::const_new();

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
        .inspect(|csrf_token| {
            CSRF_LOOKUP_CACHE.with(|cache| {
                cache
                    .borrow_mut()
                    .insert(session_token.to_string(), csrf_token.clone());
            });
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

fn configured_test_database_backend() -> TestDatabaseBackend {
    match std::env::var(TEST_DATABASE_BACKEND_ENV)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("sqlite") => TestDatabaseBackend::Sqlite,
        Some("postgres") | Some("postgresql") => TestDatabaseBackend::Postgres,
        Some("mysql") => TestDatabaseBackend::MySql,
        Some(other) => panic!(
            "unsupported {TEST_DATABASE_BACKEND_ENV} value '{other}', expected sqlite/postgres/mysql"
        ),
    }
}

async fn wait_for_database(database_url: &str) {
    let mut last_err: Option<String> = None;
    let ready = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let cfg = aster_drive::config::DatabaseConfig {
                url: database_url.to_string(),
                pool_size: 1,
                retry_count: 0,
            };
            match aster_drive::db::connect(&cfg).await {
                Ok(_) => break,
                Err(err) => {
                    last_err = Some(err.to_string());
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    })
    .await;

    if ready.is_err() {
        panic!(
            "timed out waiting for database {database_url}: {}",
            last_err.unwrap_or_else(|| "unknown error".to_string())
        );
    }
}

async fn start_postgres_test_container() -> SharedTestDatabaseContainer {
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("postgres", "16")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(5432))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "asterdrive")
        .start()
        .await
        .expect("failed to start postgres test container");
    let port = container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(5432))
        .await
        .expect("postgres test port should be exposed");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/asterdrive");

    wait_for_database(&database_url).await;

    SharedTestDatabaseContainer {
        _container: container,
        admin_database_url: database_url.clone(),
        database_url,
    }
}

async fn start_mysql_test_container() -> SharedTestDatabaseContainer {
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("mysql", "8.4")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(3306))
        .with_env_var("MYSQL_DATABASE", "asterdrive")
        .with_env_var("MYSQL_USER", "aster")
        .with_env_var("MYSQL_PASSWORD", "asterpass")
        .with_env_var("MYSQL_ROOT_PASSWORD", "rootpass")
        .start()
        .await
        .expect("failed to start mysql test container");
    let port = container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(3306))
        .await
        .expect("mysql test port should be exposed");
    let database_url = format!("mysql://aster:asterpass@127.0.0.1:{port}/asterdrive");
    let admin_database_url = format!("mysql://root:rootpass@127.0.0.1:{port}/asterdrive");

    wait_for_database(&database_url).await;

    SharedTestDatabaseContainer {
        _container: container,
        admin_database_url,
        database_url,
    }
}

async fn shared_test_database_urls(backend: TestDatabaseBackend) -> (String, String) {
    match backend {
        TestDatabaseBackend::Sqlite => {
            ("sqlite::memory:".to_string(), "sqlite::memory:".to_string())
        }
        TestDatabaseBackend::Postgres => {
            let container = POSTGRES_TEST_CONTAINER
                .get_or_init(start_postgres_test_container)
                .await;
            (
                container.admin_database_url.clone(),
                container.database_url.clone(),
            )
        }
        TestDatabaseBackend::MySql => {
            let container = MYSQL_TEST_CONTAINER
                .get_or_init(start_mysql_test_container)
                .await;
            (
                container.admin_database_url.clone(),
                container.database_url.clone(),
            )
        }
    }
}

fn sanitized_database_name_prefix(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "asterdrive".to_string()
    } else {
        trimmed.to_string()
    }
}

fn isolated_database_name(base_name: &str, max_len: usize) -> String {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let reserved = "_it_".len() + suffix.len();
    let max_prefix_len = max_len.saturating_sub(reserved).max(1);
    let prefix: String = sanitized_database_name_prefix(base_name)
        .chars()
        .take(max_prefix_len)
        .collect();

    format!("{prefix}_it_{suffix}")
}

fn database_name_from_url(url: &reqwest::Url) -> Option<String> {
    url.path_segments()
        .and_then(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .next_back()
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty())
}

fn replace_database_name(mut url: reqwest::Url, database_name: &str) -> String {
    url.set_path(&format!("/{database_name}"));
    url.to_string()
}

fn quote_database_identifier(backend: sea_orm::DbBackend, database_name: &str) -> String {
    match backend {
        sea_orm::DbBackend::Postgres => format!("\"{}\"", database_name.replace('"', "\"\"")),
        sea_orm::DbBackend::MySql => format!("`{}`", database_name.replace('`', "``")),
        _ => database_name.to_string(),
    }
}

fn quote_mysql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

async fn provision_isolated_test_database_url(
    admin_database_url: &str,
    database_url: &str,
) -> String {
    if database_url == "sqlite::memory:" || database_url.starts_with("sqlite://") {
        return database_url.to_string();
    }

    use sea_orm::ConnectionTrait;

    let admin_cfg = aster_drive::config::DatabaseConfig {
        url: admin_database_url.to_string(),
        pool_size: 1,
        retry_count: 0,
    };
    let admin_db = aster_drive::db::connect(&admin_cfg).await.unwrap();
    let backend = admin_db.get_database_backend();
    let parsed_url = reqwest::Url::parse(database_url).unwrap();
    let base_name = database_name_from_url(&parsed_url).unwrap_or_else(|| "asterdrive".to_string());

    let isolated_name = match backend {
        sea_orm::DbBackend::Postgres => isolated_database_name(&base_name, 63),
        sea_orm::DbBackend::MySql => isolated_database_name(&base_name, 64),
        _ => return database_url.to_string(),
    };

    let create_sql = format!(
        "CREATE DATABASE {}",
        quote_database_identifier(backend, &isolated_name)
    );
    admin_db.execute_unprepared(&create_sql).await.unwrap();

    if backend == sea_orm::DbBackend::MySql {
        let username = parsed_url.username();
        if !username.is_empty() {
            let grant_sql = format!(
                "GRANT ALL PRIVILEGES ON {}.* TO {}@'%'",
                quote_database_identifier(backend, &isolated_name),
                quote_mysql_string(username),
            );
            admin_db.execute_unprepared(&grant_sql).await.unwrap();
        }
    }

    replace_database_name(parsed_url, &isolated_name)
}

async fn resolve_test_database_url() -> String {
    let backend = configured_test_database_backend();
    let (admin_database_url, database_url) = shared_test_database_urls(backend).await;
    provision_isolated_test_database_url(&admin_database_url, &database_url).await
}

/// 构建一个干净的测试 AppState。
///
/// 默认使用内存 SQLite。若设置 `ASTER_TEST_DATABASE_BACKEND=postgres|mysql`，
/// 会自动启动一个共享 testcontainers 容器，并为当前测试实例分配独立数据库。
#[allow(dead_code)]
pub async fn setup() -> AppState {
    let database_url = resolve_test_database_url().await;
    setup_with_database_url(&database_url).await
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

    if matches!(name, "aster_access" | "aster_refresh")
        && let Some(csrf_token) = resp
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "aster_csrf")
            .map(|cookie| cookie.value().to_string())
    {
        remember_csrf_token(&value, &csrf_token);
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
