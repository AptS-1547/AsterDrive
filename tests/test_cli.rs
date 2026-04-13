#[macro_use]
mod common;

use actix_web::test as actix_test;
use std::process::Command;

use aster_drive::config::DatabaseConfig;
use aster_drive::db;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
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
    assert!(checkpoint_source_url.contains(
        source_db_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("source db file name should be valid utf-8")
    ));
    assert!(checkpoint_target_url.contains(
        target_db_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("target db file name should be valid utf-8")
    ));

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
