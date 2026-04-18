//! 集成测试：`cli`。

#[macro_use]
mod common;

use actix_web::test as actix_test;
use std::process::Command;

use aster_drive::config::DatabaseConfig;
use aster_drive::db;
use aster_drive::db::repository::{
    contact_verification_token_repo, file_repo, policy_repo, user_repo,
};
use aster_drive::entities::contact_verification_token;
use aster_drive::types::{VerificationChannel, VerificationPurpose};
use chrono::{Duration, Utc};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Set, Statement};
use serde_json::Value;

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

fn redact_database_url(database_url: &str) -> String {
    if database_url == "sqlite::memory:" {
        return database_url.to_string();
    }

    if database_url.starts_with("sqlite:") {
        let Some(path_and_query) = database_url.strip_prefix("sqlite://") else {
            return database_url.to_string();
        };
        let (path, query) = path_and_query
            .split_once('?')
            .map_or((path_and_query, None), |(path, query)| (path, Some(query)));
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        return match query {
            Some(query) => format!("sqlite:///.../{filename}?{query}"),
            None => format!("sqlite:///.../{filename}"),
        };
    }

    let Some((scheme, rest)) = database_url.split_once("://") else {
        return database_url.to_string();
    };
    let Some((_authority, suffix)) = rest.rsplit_once('@') else {
        return database_url.to_string();
    };

    format!("{scheme}://***@{suffix}")
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
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
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
    assert!(stdout.contains("--deep"));
    assert!(stdout.contains("--fix"));
    assert!(stdout.contains("--scope"));
    assert!(stdout.contains("--policy-id"));
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
async fn test_root_binary_doctor_warns_when_public_site_url_uses_http() {
    let database_url = setup_ready_database_url().await;

    let set_output = run_aster_drive(&[
        "config",
        "--database-url",
        &database_url,
        "set",
        "--key",
        "public_site_url",
        "--value",
        "http://drive.example.com",
    ]);
    assert!(
        set_output.status.success(),
        "set stderr: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    let output = run_aster_drive(&["doctor", "--database-url", &database_url]);
    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should be json");
    let checks = report["data"]["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    let public_site_url_check = checks
        .iter()
        .find(|check| check["name"] == "public_site_url")
        .expect("public_site_url check should exist");

    assert_eq!(public_site_url_check["status"], "warn");
    assert_eq!(
        public_site_url_check["summary"],
        "public_site_url uses insecure HTTP"
    );
    assert!(
        public_site_url_check["details"]
            .as_array()
            .is_some_and(|details| details
                .iter()
                .any(|detail| detail.as_str() == Some("configured=http://drive.example.com")))
    );
}

#[tokio::test]
async fn test_root_binary_doctor_reports_sqlite_search_acceleration_ready() {
    let database_url = setup_ready_database_url().await;

    let output = run_aster_drive(&["doctor", "--database-url", &database_url]);
    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should be json");
    let checks = report["data"]["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    let sqlite_search_check = checks
        .iter()
        .find(|check| check["name"] == "sqlite_search_acceleration")
        .expect("sqlite_search_acceleration check should exist");

    assert_eq!(sqlite_search_check["status"], "ok");
    assert_eq!(
        sqlite_search_check["summary"],
        "FTS5 trigram search acceleration ready"
    );
    assert!(
        sqlite_search_check["details"]
            .as_array()
            .is_some_and(|details| details.iter().any(|detail| detail
                .as_str()
                .is_some_and(|detail| detail.starts_with("sqlite_version="))))
    );
}

#[tokio::test]
async fn test_root_binary_doctor_deep_fix_repairs_counters() {
    let database_url = setup_database_url().await;
    let state = common::setup_with_database_url(&database_url).await;
    let db = state.db.clone();
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    let user = user_repo::find_by_email(&db, "test@example.com")
        .await
        .unwrap()
        .expect("test user should exist");
    let file = file_repo::find_by_id(&db, file_id).await.unwrap();
    let blob = file_repo::find_blob_by_id(&db, file.blob_id).await.unwrap();

    let mut user_active: aster_drive::entities::user::ActiveModel = user.into();
    user_active.storage_used = Set(0);
    user_active.update(&db).await.unwrap();

    let mut blob_active: aster_drive::entities::file_blob::ActiveModel = blob.into();
    blob_active.ref_count = Set(0);
    blob_active.updated_at = Set(Utc::now());
    blob_active.update(&db).await.unwrap();

    let output = run_aster_drive(&["doctor", "--database-url", &database_url, "--deep", "--fix"]);
    assert!(
        output.status.success(),
        "doctor --deep --fix stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should be json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["data"]["deep"], true);
    assert_eq!(report["data"]["fix"], true);

    let checks = report["data"]["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    let usage_check = checks
        .iter()
        .find(|check| check["name"] == "storage_usage_consistency")
        .expect("storage usage check should exist");
    assert_eq!(usage_check["status"], "ok");
    assert!(
        usage_check["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("fixed 1"))
    );

    let ref_count_check = checks
        .iter()
        .find(|check| check["name"] == "blob_ref_counts")
        .expect("blob ref_count check should exist");
    assert_eq!(ref_count_check["status"], "ok");
    assert!(
        ref_count_check["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("fixed 1"))
    );
}

#[tokio::test]
async fn test_root_binary_doctor_scope_and_policy_filter_limit_deep_checks() {
    let database_url = setup_database_url().await;
    let state = common::setup_with_database_url(&database_url).await;
    let db = state.db.clone();
    let policy = policy_repo::find_default(&db)
        .await
        .unwrap()
        .expect("default policy should exist");
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);
    let file_id = upload_test_file!(app, token);

    let user = user_repo::find_by_email(&db, "test@example.com")
        .await
        .unwrap()
        .expect("test user should exist");
    let file = file_repo::find_by_id(&db, file_id).await.unwrap();
    let blob = file_repo::find_blob_by_id(&db, file.blob_id).await.unwrap();

    let mut user_active: aster_drive::entities::user::ActiveModel = user.into();
    user_active.storage_used = Set(0);
    user_active.update(&db).await.unwrap();

    let mut blob_active: aster_drive::entities::file_blob::ActiveModel = blob.into();
    blob_active.ref_count = Set(0);
    blob_active.updated_at = Set(Utc::now());
    blob_active.update(&db).await.unwrap();

    let output = run_aster_drive(&[
        "doctor",
        "--database-url",
        &database_url,
        "--scope",
        "blob-ref-counts",
        "--policy-id",
        &policy.id.to_string(),
    ]);
    assert!(
        output.status.success(),
        "doctor scoped stdout: {}\ndoctor scoped stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("doctor stdout should be utf-8");
    let report: Value = serde_json::from_str(&stdout).expect("doctor output should be json");
    assert_eq!(report["data"]["deep"], true);
    assert_eq!(report["data"]["policy_id"], policy.id);
    assert!(
        report["data"]["scopes"]
            .as_array()
            .is_some_and(|scopes| scopes.len() == 1 && scopes[0] == "blob_ref_counts")
    );

    let checks = report["data"]["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "blob_ref_counts")
    );
    assert!(checks.iter().any(|check| check["name"] == "policy_filter"));
    assert!(
        !checks
            .iter()
            .any(|check| check["name"] == "storage_usage_consistency")
    );
    assert!(
        !checks
            .iter()
            .any(|check| check["name"] == "tracked_blob_objects")
    );
    assert!(
        !checks
            .iter()
            .any(|check| check["name"] == "folder_tree_integrity")
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

    let target_database_url = common::postgres_test_database_url().await;

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
        redact_database_url(&target_database_url)
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
    let source_database_url = common::postgres_test_database_url().await;
    let file_id = seed_migration_fixture(&source_database_url).await;

    let target_database_url = common::mysql_test_database_url().await;

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
    let source_database_url = common::mysql_test_database_url().await;
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
