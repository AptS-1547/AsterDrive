#[macro_use]
mod common;

use actix_web::test as actix_test;
use std::process::Command;

use aster_drive::config::DatabaseConfig;
use aster_drive::db;
use aster_drive::db::repository::{contact_verification_token_repo, user_repo};
use aster_drive::entities::contact_verification_token;
use aster_drive::types::{VerificationChannel, VerificationPurpose};
use chrono::{Duration, Utc};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Set, Statement};
use serde_json::Value;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

fn aster_drive_bin() -> &'static str {
    env!("CARGO_BIN_EXE_aster_drive")
}

async fn setup_database_url() -> String {
    let db_path =
        std::env::temp_dir().join(format!("asterdrive-cli-test-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let db = db::connect(&DatabaseConfig {
        url: url.clone(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    Migrator::up(&db, None).await.unwrap();
    url
}

async fn setup_ready_database_url() -> String {
    let db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-ready-test-{}.db",
        uuid::Uuid::new_v4()
    ));
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let _state = common::setup_with_database_url(&url).await;
    url
}

fn run_aster_drive(args: &[&str]) -> std::process::Output {
    run_aster_drive_with_env(args, &[])
}

fn run_aster_drive_with_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    Command::new(aster_drive_bin())
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .expect("aster_drive binary should run")
}

async fn wait_for_database(database_url: &str) {
    let mut last_err: Option<String> = None;
    let ready = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let cfg = DatabaseConfig {
                url: database_url.to_string(),
                pool_size: 1,
                retry_count: 0,
            };
            match db::connect(&cfg).await {
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

async fn scalar_i64(db: &DatabaseConnection, backend: DbBackend, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(backend, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index(0)
        .unwrap()
}

async fn scalar_string(db: &DatabaseConnection, backend: DbBackend, sql: &str) -> String {
    db.query_one_raw(Statement::from_string(backend, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index(0)
        .unwrap()
}

async fn seed_migration_fixture(database_url: &str) -> i64 {
    let state = common::setup_with_database_url(database_url).await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let folder_req = actix_test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "name": "Migrated Folder",
            "parent_id": null
        }))
        .to_request();
    let folder_resp = actix_test::call_service(&app, folder_req).await;
    assert_eq!(folder_resp.status(), 201);
    let folder_body: Value = actix_test::read_body_json(folder_resp).await;
    let folder_id = folder_body["data"]["id"]
        .as_i64()
        .expect("folder id should exist");

    upload_test_file_to_folder!(app, token, folder_id)
}

async fn seed_contact_verification_history(database_url: &str) {
    let db = db::connect(&DatabaseConfig {
        url: database_url.to_string(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    let user = user_repo::find_by_email(&db, "test@example.com")
        .await
        .unwrap()
        .expect("seed user should exist");
    let now = Utc::now();

    for (index, consumed_at) in [
        (1, Some(now - Duration::minutes(30))),
        (2, Some(now - Duration::minutes(20))),
        (3, None),
    ] {
        contact_verification_token_repo::create(
            &db,
            contact_verification_token::ActiveModel {
                user_id: Set(user.id),
                channel: Set(VerificationChannel::Email),
                purpose: Set(VerificationPurpose::PasswordReset),
                target: Set(user.email.clone()),
                token_hash: Set(format!("password-reset-history-{index}")),
                expires_at: Set(now + Duration::minutes(30)),
                consumed_at: Set(consumed_at),
                created_at: Set(now - Duration::minutes(40 - index as i64)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    for (index, consumed_at) in [
        (1, Some(now - Duration::minutes(50))),
        (2, Some(now - Duration::minutes(45))),
    ] {
        contact_verification_token_repo::create(
            &db,
            contact_verification_token::ActiveModel {
                user_id: Set(user.id),
                channel: Set(VerificationChannel::Email),
                purpose: Set(VerificationPurpose::ContactChange),
                target: Set(user.email.clone()),
                token_hash: Set(format!("contact-change-history-{index}")),
                expires_at: Set(now + Duration::minutes(30)),
                consumed_at: Set(consumed_at),
                created_at: Set(now - Duration::minutes(60 - index as i64)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
}

async fn assert_migrated_fixture(
    target_database_url: &str,
    target_backend: DbBackend,
    file_id: i64,
) {
    let target_db = db::connect(&DatabaseConfig {
        url: target_database_url.to_string(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    let users = scalar_i64(&target_db, target_backend, "SELECT COUNT(*) FROM users").await;
    let folders = scalar_i64(&target_db, target_backend, "SELECT COUNT(*) FROM folders").await;
    let files = scalar_i64(&target_db, target_backend, "SELECT COUNT(*) FROM files").await;
    let file_name = scalar_string(
        &target_db,
        target_backend,
        &format!("SELECT name FROM files WHERE id = {file_id}"),
    )
    .await;

    assert_eq!(users, 1);
    assert_eq!(folders, 1);
    assert_eq!(files, 1);
    assert_eq!(file_name, "test-in-folder.txt");
}

#[test]
fn test_root_binary_help_lists_config_subcommand() {
    let output = run_aster_drive(&["--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("help stdout should be utf-8");
    assert!(stdout.contains("AsterDrive server and operations CLI"));
    assert!(stdout.contains("serve"));
    assert!(stdout.contains("Start the AsterDrive server"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("Manage runtime configuration stored in system_config"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("Run offline health checks"));
    assert!(stdout.contains("database-migrate"));
    assert!(stdout.contains("Run an offline database backend migration"));
}

#[test]
fn test_root_binary_config_help_lists_runtime_config_commands() {
    let output = run_aster_drive(&["config", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("config help stdout should be utf-8");
    for command in [
        "list", "get", "set", "delete", "validate", "export", "import",
    ] {
        assert!(
            stdout.contains(command),
            "config help should mention '{command}', got: {stdout}"
        );
    }
}

#[tokio::test]
async fn test_root_binary_serve_help_is_available() {
    let output = run_aster_drive(&["serve", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("serve help stdout should be utf-8");
    assert!(stdout.contains("Start the AsterDrive server"));
}

#[tokio::test]
async fn test_root_binary_database_migrate_help_is_available() {
    let output = run_aster_drive(&["database-migrate", "--help"]);
    assert!(output.status.success());

    let stdout =
        String::from_utf8(output.stdout).expect("database-migrate help stdout should be utf-8");
    assert!(stdout.contains("offline database backend migration"));
    assert!(stdout.contains("--source-database-url"));
    assert!(stdout.contains("--target-database-url"));
    assert!(stdout.contains("--dry-run"));
    assert!(stdout.contains("--verify-only"));
}

#[tokio::test]
async fn test_root_binary_doctor_help_is_available() {
    let output = run_aster_drive(&["doctor", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("doctor help stdout should be utf-8");
    assert!(stdout.contains("offline health checks"));
    assert!(stdout.contains("--database-url"));
    assert!(stdout.contains("--output-format"));
    assert!(stdout.contains("--strict"));
}

#[tokio::test]
async fn test_root_binary_config_set_and_get_round_trip() {
    let database_url = setup_database_url().await;

    let set_output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "set",
        "--key",
        "public_site_url",
        "--value",
        " HTTPS://Drive.EXAMPLE.com/ ",
    ]);
    assert!(
        set_output.status.success(),
        "set stderr: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );
    let set_json: Value = serde_json::from_slice(&set_output.stdout).expect("set output json");
    assert_eq!(set_json["ok"], true);
    assert_eq!(set_json["data"]["value"], "https://drive.example.com");

    let get_output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "get",
        "--key",
        "public_site_url",
    ]);
    assert!(
        get_output.status.success(),
        "get stderr: {}",
        String::from_utf8_lossy(&get_output.stderr)
    );
    let get_json: Value = serde_json::from_slice(&get_output.stdout).expect("get output json");
    assert_eq!(get_json["ok"], true);
    assert_eq!(get_json["data"]["key"], "public_site_url");
    assert_eq!(get_json["data"]["value"], "https://drive.example.com");
}

#[tokio::test]
async fn test_root_binary_config_get_human_output_is_readable() {
    let database_url = setup_database_url().await;

    let set_output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "set",
        "--key",
        "public_site_url",
        "--value",
        " HTTPS://Drive.EXAMPLE.com/ ",
    ]);
    assert!(
        set_output.status.success(),
        "set stderr: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    let output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "--output-format",
        "human",
        "get",
        "--key",
        "public_site_url",
    ]);
    assert!(
        output.status.success(),
        "get stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("human stdout should be utf-8");
    assert!(stdout.contains("Configuration value"));
    assert!(stdout.contains("Key:"));
    assert!(stdout.contains("Value:"));
    assert!(stdout.contains("Source:"));
    assert!(stdout.contains("public_site_url"));
    assert!(stdout.contains("https://drive.example.com"));
    assert!(stdout.contains("[system]"));
    assert!(serde_json::from_str::<Value>(&stdout).is_err());
}

#[tokio::test]
async fn test_root_binary_config_list_human_summarizes_multiline_and_masks_sensitive() {
    let database_url = setup_database_url().await;

    let set_output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "set",
        "--key",
        "mail_smtp_password",
        "--value",
        "super-secret-password",
    ]);
    assert!(
        set_output.status.success(),
        "set stderr: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    let output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "--output-format",
        "human",
        "list",
    ]);
    assert!(
        output.status.success(),
        "list stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("human list stdout should be utf-8");
    assert!(stdout.contains("mail_smtp_password"));
    assert!(stdout.contains("[hidden sensitive value]"));
    assert!(!stdout.contains("super-secret-password"));
    assert!(stdout.contains("mail_template_register_activation_html"));
    assert!(stdout.contains("<html template:"));
    assert!(!stdout.contains("mail_template_register_activation_html = <!doctype html>"));
    assert!(stdout.contains("frontend_preview_apps_json"));
    assert!(stdout.contains("<json value:"));
    assert!(!stdout.contains("frontend_preview_apps_json = {"));
}

#[tokio::test]
async fn test_root_binary_config_human_output_supports_forced_color() {
    let database_url = setup_database_url().await;

    let output = run_aster_drive_with_env(
        &[
            "config",
            "--database-url",
            &database_url,
            "--output-format",
            "human",
            "list",
        ],
        &[("CLICOLOR_FORCE", "1")],
    );
    assert!(
        output.status.success(),
        "list stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("forced-color stdout should be utf-8");
    assert!(stdout.contains("\u{1b}["));
    assert!(stdout.contains("Configuration list"));
}

#[tokio::test]
async fn test_root_binary_doctor_defaults_to_json_output() {
    let database_url = setup_ready_database_url().await;

    let output = run_aster_drive(&["doctor", "--database-url", &database_url]);
    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should be json");
    assert_eq!(report["ok"], true);
    let redacted_database_url = report["data"]["database_url"]
        .as_str()
        .expect("doctor database url should be a string");
    assert!(redacted_database_url.starts_with("sqlite:///.../asterdrive-cli-ready-test-"));
    assert!(redacted_database_url.ends_with(".db?mode=rwc"));
    assert_eq!(report["data"]["status"], "warn");
    assert_eq!(report["data"]["summary"]["fail"], 0);
    assert!(
        report["data"]["summary"]["warn"]
            .as_u64()
            .expect("warn count should exist")
            >= 1
    );
}

#[tokio::test]
async fn test_root_binary_doctor_human_output_is_readable() {
    let database_url = setup_ready_database_url().await;

    let output = run_aster_drive(&[
        "doctor",
        "--database-url",
        &database_url,
        "--output-format",
        "human",
    ]);
    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor human stdout should be utf-8");
    assert!(stdout.contains("System doctor"));
    assert!(stdout.contains("Database:"));
    assert!(stdout.contains("Mode:"));
    assert!(stdout.contains("Status:"));
    assert!(stdout.contains("Checks:"));
    assert!(stdout.contains("Database connection"));
    assert!(stdout.contains("Mail configuration"));
    assert!(stdout.contains("hint:"));
    assert!(serde_json::from_str::<Value>(&stdout).is_err());
}

#[tokio::test]
async fn test_root_binary_doctor_human_output_supports_forced_color() {
    let database_url = setup_ready_database_url().await;

    let output = run_aster_drive_with_env(
        &[
            "doctor",
            "--database-url",
            &database_url,
            "--output-format",
            "human",
        ],
        &[("CLICOLOR_FORCE", "1")],
    );
    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout =
        String::from_utf8(output.stdout).expect("doctor forced-color stdout should be utf-8");
    assert!(stdout.contains("\u{1b}["));
    assert!(stdout.contains("System doctor"));
}

#[tokio::test]
async fn test_root_binary_doctor_strict_turns_warnings_into_nonzero_exit() {
    let database_url = setup_ready_database_url().await;

    let output = run_aster_drive(&["doctor", "--database-url", &database_url, "--strict"]);
    assert!(
        !output.status.success(),
        "doctor --strict should fail when warnings are present"
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should stay json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["data"]["strict"], true);
    assert_eq!(report["data"]["status"], "fail");
    assert_eq!(report["data"]["summary"]["fail"], 0);
    assert!(
        report["data"]["summary"]["warn"]
            .as_u64()
            .expect("warn count should exist")
            >= 1
    );
}

#[tokio::test]
async fn test_root_binary_doctor_exits_nonzero_when_storage_policy_setup_is_missing() {
    let database_url = setup_database_url().await;

    let output = run_aster_drive(&["doctor", "--database-url", &database_url]);
    assert!(
        !output.status.success(),
        "doctor should fail on incomplete setup"
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should stay json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["data"]["status"], "fail");
    let checks = report["data"]["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    assert!(
        checks
            .iter()
            .any(|check| { check["name"] == "storage_policies" && check["status"] == "fail" })
    );
}

#[tokio::test]
async fn test_root_binary_config_delete_rejects_system_config_key() {
    let database_url = setup_database_url().await;

    let output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "delete",
        "--key",
        "public_site_url",
    ]);
    assert!(
        !output.status.success(),
        "delete should fail for system config"
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    let err_json: Value = serde_json::from_str(&stderr).expect("error output json");
    assert_eq!(err_json["ok"], false);
    assert_eq!(err_json["error"]["code"], "E013");
    assert!(
        err_json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cannot delete system configuration")
    );
}

#[tokio::test]
async fn test_root_binary_database_migrate_sqlite_to_postgres_happy_path() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-migrate-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let file_id = seed_migration_fixture(&source_database_url).await;

    let container = GenericImage::new("postgres", "16")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(5432))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "asterdrive")
        .start()
        .await
        .expect("failed to start postgres container");
    let port = container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(5432))
        .await
        .expect("postgres port should be exposed");
    let target_database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/asterdrive");
    wait_for_database(&target_database_url).await;

    let output = run_aster_drive(&[
        "database-migrate",
        "--source-database-url",
        &source_database_url,
        "--target-database-url",
        &target_database_url,
    ]);
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&output.stdout).expect("database-migrate output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["mode"], "apply");
    assert_eq!(output_json["data"]["ready_to_cutover"], true);
    assert_eq!(output_json["data"]["rolled_back"], false);
    assert_eq!(output_json["data"]["resume"]["enabled"], true);
    assert_eq!(output_json["data"]["resume"]["resumed"], false);
    assert_eq!(
        output_json["data"]["source"]["database_url"],
        format!(
            "sqlite:///.../{}?mode=rwc",
            source_db_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("source db file name should be valid utf-8")
        )
    );
    assert_eq!(
        output_json["data"]["target"]["database_url"],
        format!("postgres://***@127.0.0.1:{port}/asterdrive")
    );

    let target_db = db::connect(&DatabaseConfig {
        url: target_database_url.clone(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    let checkpoint_source_url = scalar_string(
        &target_db,
        DbBackend::Postgres,
        "SELECT source_database_url FROM aster_cli_database_migrations LIMIT 1",
    )
    .await;
    let checkpoint_target_url = scalar_string(
        &target_db,
        DbBackend::Postgres,
        "SELECT target_database_url FROM aster_cli_database_migrations LIMIT 1",
    )
    .await;
    assert_eq!(
        checkpoint_source_url,
        output_json["data"]["source"]["database_url"]
    );
    assert_eq!(
        checkpoint_target_url,
        output_json["data"]["target"]["database_url"]
    );

    assert_migrated_fixture(&target_database_url, DbBackend::Postgres, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_postgres_to_mysql_with_progress() {
    let source_container = GenericImage::new("postgres", "16")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(5432))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "asterdrive")
        .start()
        .await
        .expect("failed to start postgres source container");
    let source_port = source_container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(5432))
        .await
        .expect("postgres source port should be exposed");
    let source_database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{source_port}/asterdrive");
    wait_for_database(&source_database_url).await;
    let file_id = seed_migration_fixture(&source_database_url).await;

    let target_container = GenericImage::new("mysql", "8.4")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(3306))
        .with_env_var("MYSQL_DATABASE", "asterdrive")
        .with_env_var("MYSQL_USER", "aster")
        .with_env_var("MYSQL_PASSWORD", "asterpass")
        .with_env_var("MYSQL_ROOT_PASSWORD", "rootpass")
        .start()
        .await
        .expect("failed to start mysql target container");
    let target_port = target_container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(3306))
        .await
        .expect("mysql target port should be exposed");
    let target_database_url = format!("mysql://aster:asterpass@127.0.0.1:{target_port}/asterdrive");
    wait_for_database(&target_database_url).await;

    let output = run_aster_drive_with_env(
        &[
            "database-migrate",
            "--source-database-url",
            &source_database_url,
            "--target-database-url",
            &target_database_url,
        ],
        &[
            ("ASTER_CLI_PROGRESS", "1"),
            ("ASTER_CLI_COPY_BATCH_SIZE", "1"),
        ],
    );
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&output.stdout).expect("database-migrate output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["ready_to_cutover"], true);
    assert_eq!(output_json["data"]["resume"]["resumed"], false);

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("[database-migrate] data_copy:"));

    assert_migrated_fixture(&target_database_url, DbBackend::MySql, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_mysql_to_sqlite_happy_path() {
    let source_container = GenericImage::new("mysql", "8.4")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(3306))
        .with_env_var("MYSQL_DATABASE", "asterdrive")
        .with_env_var("MYSQL_USER", "aster")
        .with_env_var("MYSQL_PASSWORD", "asterpass")
        .with_env_var("MYSQL_ROOT_PASSWORD", "rootpass")
        .start()
        .await
        .expect("failed to start mysql source container");
    let source_port = source_container
        .get_host_port_ipv4(testcontainers::core::IntoContainerPort::tcp(3306))
        .await
        .expect("mysql source port should be exposed");
    let source_database_url = format!("mysql://aster:asterpass@127.0.0.1:{source_port}/asterdrive");
    wait_for_database(&source_database_url).await;
    let file_id = seed_migration_fixture(&source_database_url).await;

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-migrate-target-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}?mode=rwc", target_db_path.display());

    let output = run_aster_drive(&[
        "database-migrate",
        "--source-database-url",
        &source_database_url,
        "--target-database-url",
        &target_database_url,
    ]);
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&output.stdout).expect("database-migrate output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["ready_to_cutover"], true);

    assert_migrated_fixture(&target_database_url, DbBackend::Sqlite, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_sqlite_resume_from_checkpoint() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-resume-source-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let file_id = seed_migration_fixture(&source_database_url).await;

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-resume-target-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}?mode=rwc", target_db_path.display());

    let first_output = run_aster_drive_with_env(
        &[
            "database-migrate",
            "--source-database-url",
            &source_database_url,
            "--target-database-url",
            &target_database_url,
        ],
        &[
            ("ASTER_CLI_COPY_BATCH_SIZE", "1"),
            ("ASTER_CLI_FAIL_AFTER_BATCHES", "1"),
        ],
    );
    assert!(
        !first_output.status.success(),
        "first migration should fail to exercise resume"
    );
    let error_json: Value =
        serde_json::from_slice(&first_output.stderr).expect("error stderr should stay json");
    assert_eq!(error_json["ok"], false);
    assert!(
        error_json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("forced failure")
    );

    let target_db = db::connect(&DatabaseConfig {
        url: target_database_url.clone(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    let checkpoint_rows = scalar_i64(
        &target_db,
        DbBackend::Sqlite,
        "SELECT COUNT(*) FROM aster_cli_database_migrations",
    )
    .await;
    assert_eq!(checkpoint_rows, 1);
    let checkpoint_source_url = scalar_string(
        &target_db,
        DbBackend::Sqlite,
        "SELECT source_database_url FROM aster_cli_database_migrations LIMIT 1",
    )
    .await;
    let checkpoint_target_url = scalar_string(
        &target_db,
        DbBackend::Sqlite,
        "SELECT target_database_url FROM aster_cli_database_migrations LIMIT 1",
    )
    .await;
    assert_ne!(checkpoint_source_url, source_database_url);
    assert_ne!(checkpoint_target_url, target_database_url);
    assert!(checkpoint_source_url.contains("/.../"));
    assert!(checkpoint_target_url.contains("/.../"));
    assert!(
        checkpoint_source_url.contains(
            source_db_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("source db file name should be valid utf-8")
        )
    );
    assert!(
        checkpoint_target_url.contains(
            target_db_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("target db file name should be valid utf-8")
        )
    );

    let second_output = run_aster_drive_with_env(
        &[
            "database-migrate",
            "--source-database-url",
            &source_database_url,
            "--target-database-url",
            &target_database_url,
        ],
        &[("ASTER_CLI_COPY_BATCH_SIZE", "1")],
    );
    assert!(
        second_output.status.success(),
        "resume stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&second_output.stdout).expect("resume output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["ready_to_cutover"], true);
    assert_eq!(output_json["data"]["resume"]["enabled"], true);
    assert_eq!(output_json["data"]["resume"]["resumed"], true);

    assert_migrated_fixture(&target_database_url, DbBackend::Sqlite, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_sqlite_urls_without_mode_default_to_rwc() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-source-no-mode-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url_with_mode = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let file_id = seed_migration_fixture(&source_database_url_with_mode).await;
    let source_database_url = format!("sqlite://{}", source_db_path.display());

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-target-no-mode-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}", target_db_path.display());

    let output = run_aster_drive(&[
        "database-migrate",
        "--source-database-url",
        &source_database_url,
        "--target-database-url",
        &target_database_url,
    ]);
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&output.stdout).expect("database-migrate output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["ready_to_cutover"], true);

    assert_migrated_fixture(
        &format!("{target_database_url}?mode=rwc"),
        DbBackend::Sqlite,
        file_id,
    )
    .await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_allows_consumed_contact_verification_history() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-contact-history-source-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let file_id = seed_migration_fixture(&source_database_url).await;
    seed_contact_verification_history(&source_database_url).await;

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-contact-history-target-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}?mode=rwc", target_db_path.display());

    let output = run_aster_drive(&[
        "database-migrate",
        "--source-database-url",
        &source_database_url,
        "--target-database-url",
        &target_database_url,
    ]);
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_json: Value =
        serde_json::from_slice(&output.stdout).expect("database-migrate output should be json");
    assert_eq!(output_json["ok"], true);
    assert_eq!(output_json["data"]["ready_to_cutover"], true);
    assert_eq!(
        output_json["data"]["verification"]["unique_conflicts"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let target_db = db::connect(&DatabaseConfig {
        url: target_database_url.clone(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
    .unwrap();
    let active_password_reset_tokens = scalar_i64(
        &target_db,
        DbBackend::Sqlite,
        "SELECT COUNT(*) FROM contact_verification_tokens \
         WHERE channel = 'email' AND purpose = 'password_reset' AND consumed_at IS NULL",
    )
    .await;
    let historical_contact_change_tokens = scalar_i64(
        &target_db,
        DbBackend::Sqlite,
        "SELECT COUNT(*) FROM contact_verification_tokens \
         WHERE channel = 'email' AND purpose = 'contact_change' AND consumed_at IS NOT NULL",
    )
    .await;
    assert_eq!(active_password_reset_tokens, 1);
    assert_eq!(historical_contact_change_tokens, 2);

    assert_migrated_fixture(&target_database_url, DbBackend::Sqlite, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_human_output_is_readable() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-human-source-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let file_id = seed_migration_fixture(&source_database_url).await;

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-human-target-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}?mode=rwc", target_db_path.display());

    let output = run_aster_drive_with_env(
        &[
            "database-migrate",
            "--output-format",
            "human",
            "--source-database-url",
            &source_database_url,
            "--target-database-url",
            &target_database_url,
        ],
        &[("ASTER_CLI_PROGRESS", "1")],
    );
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("human stdout should be utf-8");
    assert!(stdout.contains("Database migration complete"));
    assert!(stdout.contains("Stages:"));
    assert!(stdout.contains("Source:"));
    assert!(stdout.contains("Target:"));
    assert!(stdout.contains("Cutover:"));
    assert!(stdout.contains("[OK] ready"));
    assert!(stdout.contains("Verification:"));
    assert!(serde_json::from_str::<Value>(&stdout).is_err());

    let stderr = String::from_utf8(output.stderr).expect("human stderr should be utf-8");
    assert!(stderr.contains("[database-migrate] data_copy: ["));

    assert_migrated_fixture(&target_database_url, DbBackend::Sqlite, file_id).await;
}

#[tokio::test]
async fn test_root_binary_database_migrate_human_output_supports_forced_color() {
    let source_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-human-color-source-{}.db",
        uuid::Uuid::new_v4()
    ));
    let source_database_url = format!("sqlite://{}?mode=rwc", source_db_path.display());
    let _ = seed_migration_fixture(&source_database_url).await;

    let target_db_path = std::env::temp_dir().join(format!(
        "asterdrive-cli-human-color-target-{}.db",
        uuid::Uuid::new_v4()
    ));
    let target_database_url = format!("sqlite://{}?mode=rwc", target_db_path.display());

    let output = run_aster_drive_with_env(
        &[
            "database-migrate",
            "--output-format",
            "human",
            "--source-database-url",
            &source_database_url,
            "--target-database-url",
            &target_database_url,
        ],
        &[("ASTER_CLI_PROGRESS", "1"), ("CLICOLOR_FORCE", "1")],
    );
    assert!(
        output.status.success(),
        "database-migrate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("forced-color stdout should be utf-8");
    let stderr = String::from_utf8(output.stderr).expect("forced-color stderr should be utf-8");
    assert!(stdout.contains("\u{1b}["));
    assert!(stderr.contains("\u{1b}["));
}
