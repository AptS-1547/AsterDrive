//! Background task integration tests

#[macro_use]
mod common;

use actix_web::{App, test, web};
use chrono::{Duration, Utc};
use sea_orm::Set;
use serde_json::Value;
use std::io::{Cursor, Read, Write};

use aster_drive::db::repository::background_task_repo;
use aster_drive::entities::background_task;
use aster_drive::services::task_service::{self, RuntimeTaskRunOutcome};
use aster_drive::types::{BackgroundTaskKind, BackgroundTaskStatus, StoredTaskPayload};

macro_rules! register_user {
    ($app:expr, $db:expr, $mail_sender:expr, $username:expr, $email:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/register")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "username": $username,
                "email": $email,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 201);
        let _body: Value = test::read_body_json(resp).await;
        let _ = confirm_latest_contact_verification!($app, $db, $mail_sender);
    }};
}

macro_rules! login_user {
    ($app:expr, $identifier:expr, $password:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .set_json(serde_json::json!({
                "identifier": $identifier,
                "password": $password
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), 200);
        common::extract_cookie(&resp, "aster_access").unwrap()
    }};
}

macro_rules! multipart_request {
    ($uri:expr, $token:expr, $filename:expr, $content:expr $(,)?) => {{
        let boundary = "----TaskBoundary123";
        let payload = format!(
            "------TaskBoundary123\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             {content}\r\n\
             ------TaskBoundary123--\r\n",
            filename = $filename,
            content = $content,
        );

        test::TestRequest::post()
            .uri($uri)
            .insert_header(("Cookie", common::access_cookie_header(&$token)))
            .insert_header(common::csrf_header_for(&$token))
            .insert_header((
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(payload)
            .to_request()
    }};
}

fn zip_entry_names(bytes: &[u8]) -> Vec<String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip archive should be readable");
    let mut names = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        names.push(
            archive
                .by_index(index)
                .expect("zip entry should exist")
                .name()
                .to_string(),
        );
    }
    names.sort();
    names
}

fn read_zip_entry_text(bytes: &[u8], name: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip archive should be readable");
    let mut entry = archive.by_name(name).expect("zip entry should exist");
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .expect("zip entry should be readable as utf-8 text");
    content
}

fn read_archive_download_path(body: &Value) -> String {
    body["data"]["token"]
        .as_str()
        .expect("ticket token should exist");
    body["data"]["download_path"]
        .as_str()
        .expect("download path should exist")
        .to_string()
}

fn read_task_result(body: &Value) -> Value {
    body["data"]["result"].clone()
}

fn read_task_steps(body: &Value) -> Vec<(String, String)> {
    body["data"]["steps"]
        .as_array()
        .expect("task steps should exist")
        .iter()
        .map(|step| {
            (
                step["key"]
                    .as_str()
                    .expect("task step key should exist")
                    .to_string(),
                step["status"]
                    .as_str()
                    .expect("task step status should exist")
                    .to_string(),
            )
        })
        .collect()
}

fn assert_task_steps(body: &Value, expected: &[(&str, &str)]) {
    let actual = read_task_steps(body);
    let expected = expected
        .iter()
        .map(|(key, status)| (key.to_string(), status.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn create_zip_bytes(entries: &[(&str, Option<&[u8]>)]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let file_options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let dir_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (path, content) in entries {
        match content {
            Some(bytes) => {
                zip.start_file(*path, file_options)
                    .expect("zip entry should start");
                zip.write_all(bytes).expect("zip entry should be writable");
            }
            None => {
                zip.add_directory(*path, dir_options)
                    .expect("zip directory should be writable");
            }
        }
    }

    zip.finish().expect("zip writer should finish").into_inner()
}

async fn insert_processing_task(
    state: &aster_drive::runtime::AppState,
    processing_started_at: chrono::DateTime<chrono::Utc>,
    last_heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
) -> i64 {
    let now = Utc::now();
    background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Processing),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("heartbeat-test".to_string()),
            payload_json: Set(StoredTaskPayload("{}".to_string())),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now),
            processing_token: Set(0),
            processing_started_at: Set(Some(processing_started_at)),
            last_heartbeat_at: Set(last_heartbeat_at),
            started_at: Set(Some(processing_started_at)),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .expect("processing task should be inserted")
    .id
}

async fn insert_pending_dispatch_task(
    state: &aster_drive::runtime::AppState,
    max_attempts: i32,
) -> background_task::Model {
    let now = Utc::now();
    background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::SystemRuntime),
            status: Set(BackgroundTaskStatus::Pending),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("dispatch-test".to_string()),
            payload_json: Set(StoredTaskPayload(
                r#"{"task_name":"dispatch-test"}"#.to_string(),
            )),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(max_attempts),
            next_run_at: Set(now - Duration::seconds(1)),
            processing_token: Set(0),
            processing_started_at: Set(None),
            last_heartbeat_at: Set(None),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .expect("pending dispatch task should be inserted")
}

async fn assert_response_status(
    resp: actix_web::dev::ServiceResponse,
    expected: actix_web::http::StatusCode,
) -> actix_web::dev::ServiceResponse {
    let status = resp.status();
    if status != expected {
        let body = test::read_body(resp).await;
        panic!(
            "expected status {}, got {} with body: {}",
            expected,
            status,
            String::from_utf8_lossy(&body)
        );
    }
    resp
}

#[actix_web::test]
async fn test_processing_task_claimability_prefers_last_heartbeat_at() {
    let state = common::setup().await;
    let now = Utc::now();
    let stale_before = now - Duration::seconds(60);
    let processing_started_at = now - Duration::seconds(180);

    let fresh_heartbeat_task = insert_processing_task(
        &state,
        processing_started_at,
        Some(now - Duration::seconds(5)),
    )
    .await;
    let legacy_stale_task = insert_processing_task(&state, processing_started_at, None).await;
    let stale_heartbeat_task = insert_processing_task(
        &state,
        processing_started_at,
        Some(now - Duration::seconds(120)),
    )
    .await;

    let claimable = background_task_repo::list_claimable(&state.db, now, stale_before, 10)
        .await
        .expect("claimable task list should load");
    let ids = claimable.iter().map(|task| task.id).collect::<Vec<_>>();

    assert!(!ids.contains(&fresh_heartbeat_task));
    assert!(ids.contains(&legacy_stale_task));
    assert!(ids.contains(&stale_heartbeat_task));
}

#[actix_web::test]
async fn test_processing_task_claimability_prefers_explicit_lease_expiry_when_present() {
    let state = common::setup().await;
    let now = Utc::now();
    let stale_before = now - Duration::seconds(60);

    let fresh_lease = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Processing),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("fresh-lease".to_string()),
            payload_json: Set(StoredTaskPayload("{}".to_string())),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now - Duration::seconds(120)),
            processing_token: Set(1),
            processing_started_at: Set(Some(now - Duration::seconds(180))),
            last_heartbeat_at: Set(Some(now - Duration::seconds(120))),
            lease_expires_at: Set(Some(now + Duration::seconds(30))),
            started_at: Set(Some(now - Duration::seconds(180))),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now - Duration::seconds(180)),
            updated_at: Set(now - Duration::seconds(120)),
            ..Default::default()
        },
    )
    .await
    .expect("fresh explicit lease task should be inserted");

    let expired_lease = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Processing),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("expired-lease".to_string()),
            payload_json: Set(StoredTaskPayload("{}".to_string())),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now - Duration::seconds(120)),
            processing_token: Set(1),
            processing_started_at: Set(Some(now - Duration::seconds(30))),
            last_heartbeat_at: Set(Some(now - Duration::seconds(5))),
            lease_expires_at: Set(Some(now - Duration::seconds(1))),
            started_at: Set(Some(now - Duration::seconds(30))),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now - Duration::seconds(30)),
            updated_at: Set(now - Duration::seconds(5)),
            ..Default::default()
        },
    )
    .await
    .expect("expired explicit lease task should be inserted");

    let claimable = background_task_repo::list_claimable(&state.db, now, stale_before, 10)
        .await
        .expect("claimable task list should load");
    let ids = claimable.iter().map(|task| task.id).collect::<Vec<_>>();

    assert!(!ids.contains(&fresh_lease.id));
    assert!(ids.contains(&expired_lease.id));
}

#[actix_web::test]
async fn test_touch_heartbeat_refreshes_processing_task_liveness() {
    let state = common::setup().await;
    let now = Utc::now();
    let old_heartbeat = now - Duration::seconds(120);
    let task_id =
        insert_processing_task(&state, now - Duration::seconds(180), Some(old_heartbeat)).await;

    let touched_at = Utc::now();
    let touched = background_task_repo::touch_heartbeat(
        &state.db,
        task_id,
        0,
        touched_at,
        touched_at + Duration::seconds(60),
    )
    .await
    .expect("heartbeat touch should succeed");
    assert!(touched);

    let task = background_task_repo::find_by_id(&state.db, task_id)
        .await
        .expect("task should still exist");
    assert!(
        task.last_heartbeat_at
            .expect("heartbeat timestamp should exist")
            > old_heartbeat
    );
    assert!(
        task.lease_expires_at
            .expect("lease expiry should exist after heartbeat")
            > touched_at
    );
}

#[actix_web::test]
async fn test_processing_token_fences_stale_worker_updates_after_reclaim() {
    let state = common::setup().await;
    let now = Utc::now();
    let stale_before = now - Duration::seconds(60);
    let task = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Processing),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("fencing-test".to_string()),
            payload_json: Set(StoredTaskPayload("{}".to_string())),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now - Duration::seconds(120)),
            processing_token: Set(1),
            processing_started_at: Set(Some(now - Duration::seconds(180))),
            last_heartbeat_at: Set(Some(now - Duration::seconds(120))),
            started_at: Set(Some(now - Duration::seconds(180))),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now - Duration::seconds(180)),
            updated_at: Set(now - Duration::seconds(120)),
            ..Default::default()
        },
    )
    .await
    .expect("processing task with token should be inserted");

    let reclaimed = background_task_repo::try_claim(
        &state.db,
        task.id,
        task.processing_token,
        Utc::now(),
        stale_before,
        task.processing_token + 1,
        Utc::now() + Duration::seconds(60),
    )
    .await
    .expect("stale processing task should be reclaimable");
    assert!(reclaimed);

    let stale_touched = background_task_repo::touch_heartbeat(
        &state.db,
        task.id,
        task.processing_token,
        Utc::now(),
        Utc::now() + Duration::seconds(60),
    )
    .await
    .expect("stale worker heartbeat should be rejected");
    assert!(!stale_touched);

    let stale_progress = background_task_repo::mark_progress(
        &state.db,
        background_task_repo::TaskProgressUpdate {
            id: task.id,
            processing_token: task.processing_token,
            now: Utc::now(),
            lease_expires_at: Utc::now() + Duration::seconds(60),
            current: 1,
            total: 2,
            status_text: Some("stale update"),
            steps_json: None,
        },
    )
    .await
    .expect("stale worker progress update should be rejected");
    assert!(!stale_progress);

    let fresh_touched = background_task_repo::touch_heartbeat(
        &state.db,
        task.id,
        task.processing_token + 1,
        Utc::now(),
        Utc::now() + Duration::seconds(60),
    )
    .await
    .expect("current worker heartbeat should still succeed");
    assert!(fresh_touched);

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("reclaimed task should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Processing);
    assert_eq!(stored.processing_token, task.processing_token + 1);
    assert!(stored.lease_expires_at.is_some());
}

#[actix_web::test]
async fn test_dispatch_due_reclaims_stale_processing_task_with_new_token() {
    let state = common::setup().await;
    let now = Utc::now();
    let task = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::SystemRuntime),
            status: Set(BackgroundTaskStatus::Processing),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("stale-dispatch-test".to_string()),
            payload_json: Set(StoredTaskPayload(
                r#"{"task_name":"stale-dispatch-test"}"#.to_string(),
            )),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(3),
            next_run_at: Set(now - Duration::seconds(120)),
            processing_token: Set(4),
            processing_started_at: Set(Some(now - Duration::seconds(180))),
            last_heartbeat_at: Set(Some(now - Duration::seconds(120))),
            started_at: Set(Some(now - Duration::seconds(180))),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now - Duration::seconds(180)),
            updated_at: Set(now - Duration::seconds(120)),
            ..Default::default()
        },
    )
    .await
    .expect("stale processing dispatch task should be inserted");

    let stats = task_service::dispatch_due(&state)
        .await
        .expect("dispatch should reclaim stale processing task");

    assert_eq!(stats.claimed, 1);
    assert_eq!(stats.retried, 1);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.succeeded, 0);

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("reclaimed task should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Retry);
    assert_eq!(stored.processing_token, 5);
    assert_eq!(stored.attempt_count, 1);
    assert!(stored.processing_started_at.is_none());
    assert!(stored.last_heartbeat_at.is_none());
    assert!(stored.lease_expires_at.is_none());
    assert!(
        stored
            .last_error
            .as_deref()
            .expect("reclaimed stale task should record error")
            .contains("should not be dispatched")
    );
}

#[actix_web::test]
async fn test_cleanup_expired_keeps_terminal_task_records() {
    let state = common::setup().await;
    let now = Utc::now();
    let task = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Succeeded),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("expired-task".to_string()),
            payload_json: Set(StoredTaskPayload("{}".to_string())),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(1),
            progress_total: Set(1),
            status_text: Set(Some("finished".to_string())),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now - Duration::hours(2)),
            processing_started_at: Set(None),
            last_heartbeat_at: Set(None),
            started_at: Set(Some(now - Duration::hours(2))),
            finished_at: Set(Some(now - Duration::hours(2))),
            last_error: Set(None),
            expires_at: Set(now - Duration::hours(1)),
            created_at: Set(now - Duration::hours(2)),
            updated_at: Set(now - Duration::hours(2)),
            ..Default::default()
        },
    )
    .await
    .expect("expired task should be inserted");

    let task_temp_dir =
        aster_drive::utils::paths::task_temp_dir(&state.config.server.temp_dir, task.id);
    std::fs::create_dir_all(&task_temp_dir).expect("task temp dir should be created");
    std::fs::write(format!("{task_temp_dir}/artifact.tmp"), b"expired")
        .expect("task temp artifact should be written");

    let cleaned = task_service::cleanup_expired(&state)
        .await
        .expect("task cleanup should succeed");
    assert_eq!(cleaned, 1);
    assert!(
        !std::path::Path::new(&task_temp_dir).exists(),
        "expired task temp dir should be removed"
    );

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("expired task record should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Succeeded);

    let cleaned_again = task_service::cleanup_expired(&state)
        .await
        .expect("repeated task cleanup should still succeed");
    assert_eq!(cleaned_again, 0);
}

#[actix_web::test]
async fn test_record_runtime_task_run_persists_system_runtime_event() {
    let state = common::setup().await;
    aster_drive::services::config_service::set(&state, "background_task_max_attempts", "5", 1)
        .await
        .expect("background task max attempts config should update");
    let started_at = Utc::now() - Duration::seconds(2);
    let finished_at = Utc::now();

    let recorded = task_service::record_runtime_task_run(
        &state,
        "trash-cleanup",
        started_at,
        finished_at,
        &RuntimeTaskRunOutcome::succeeded(Some("cleaned up 3 expired trash entries".to_string())),
    )
    .await
    .expect("runtime task event should be recorded")
    .expect("runtime task event should not be quiet");

    assert_eq!(recorded.kind, BackgroundTaskKind::SystemRuntime);
    assert_eq!(recorded.status, BackgroundTaskStatus::Succeeded);
    assert_eq!(recorded.display_name, "Trash cleanup");
    assert_eq!(recorded.max_attempts, 1);
    assert!(recorded.lease_expires_at.is_none());
    assert_eq!(
        serde_json::from_str::<Value>(recorded.payload_json.as_ref())
            .expect("runtime payload should be valid json")["task_name"],
        "trash-cleanup"
    );
    let result = serde_json::from_str::<Value>(recorded.result_json.as_ref().unwrap().as_ref())
        .expect("runtime result should be valid json");
    assert_eq!(result["summary"], "cleaned up 3 expired trash entries");
    assert!(result["duration_ms"].as_i64().unwrap() >= 0);
}

#[actix_web::test]
async fn test_record_runtime_task_run_skips_quiet_outcome() {
    let state = common::setup().await;
    let started_at = Utc::now() - Duration::seconds(1);
    let finished_at = Utc::now();

    let recorded = task_service::record_runtime_task_run(
        &state,
        "background-task-dispatch",
        started_at,
        finished_at,
        &RuntimeTaskRunOutcome::quiet(),
    )
    .await
    .expect("quiet runtime task handling should succeed");

    assert!(recorded.is_none());
    let recent = background_task_repo::list_recent(&state.db, 10)
        .await
        .expect("recent background tasks should load");
    assert!(recent.is_empty());
}

#[actix_web::test]
async fn test_dispatch_due_retries_failed_claimed_task_when_attempts_remain() {
    let state = common::setup().await;
    let task = insert_pending_dispatch_task(&state, 3).await;

    let stats = task_service::dispatch_due(&state)
        .await
        .expect("dispatch should succeed");

    assert_eq!(stats.claimed, 1);
    assert_eq!(stats.retried, 1);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.succeeded, 0);

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("retried task should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Retry);
    assert_eq!(stored.attempt_count, 1);
    assert!(stored.finished_at.is_none());
    assert!(stored.processing_started_at.is_none());
    assert!(stored.last_heartbeat_at.is_none());
    assert!(stored.next_run_at > Utc::now());
    assert!(
        stored
            .last_error
            .as_deref()
            .expect("retry task should record last error")
            .contains("should not be dispatched")
    );
}

#[actix_web::test]
async fn test_dispatch_due_marks_task_failed_after_max_attempts() {
    let state = common::setup().await;
    let task = insert_pending_dispatch_task(&state, 1).await;

    let stats = task_service::dispatch_due(&state)
        .await
        .expect("dispatch should succeed");

    assert_eq!(stats.claimed, 1);
    assert_eq!(stats.retried, 0);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.succeeded, 0);

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("failed task should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Failed);
    assert_eq!(stored.attempt_count, 1);
    assert!(stored.finished_at.is_some());
    assert!(stored.processing_started_at.is_none());
    assert!(stored.last_heartbeat_at.is_none());
    assert!(
        stored
            .last_error
            .as_deref()
            .expect("failed task should record last error")
            .contains("should not be dispatched")
    );
}

#[actix_web::test]
async fn test_personal_archive_stream_preserves_empty_folders() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "taskowner",
        "taskowner@example.com",
        "password123"
    );
    let token = login_user!(app, "taskowner", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "bundle", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let bundle_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "docs", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let docs_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "empty", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = multipart_request!(
        &format!("/api/v1/files/upload?folder_id={docs_id}"),
        &token,
        "note.txt",
        "hello from archive task",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/archive-download")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [],
            "folder_ids": [bundle_id],
            "archive_name": "bundle-export"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let download_path = read_archive_download_path(&body);

    let req = test::TestRequest::get()
        .uri(&download_path)
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    assert_eq!(
        resp.headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok()),
        Some("application/zip")
    );
    let zip_bytes = test::read_body(resp).await;
    let names = zip_entry_names(&zip_bytes);
    assert_eq!(
        names,
        vec![
            "bundle/",
            "bundle/docs/",
            "bundle/docs/note.txt",
            "bundle/empty/",
        ]
    );
    assert_eq!(
        read_zip_entry_text(&zip_bytes, "bundle/docs/note.txt"),
        "hello from archive task"
    );
}

#[actix_web::test]
async fn test_team_archive_stream_is_scoped_to_team_routes() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "teamowner",
        "teamowner@example.com",
        "password123"
    );
    let token = login_user!(app, "teamowner", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "Ops Team" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = multipart_request!(
        &format!("/api/v1/teams/{team_id}/files/upload"),
        &token,
        "team.txt",
        "team archive payload",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/batch/archive-download"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [file_id],
            "folder_ids": [],
            "archive_name": "ops-export"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let download_path = read_archive_download_path(&body);

    let req = test::TestRequest::get()
        .uri(&download_path)
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    let zip_bytes = test::read_body(resp).await;
    assert_eq!(zip_entry_names(&zip_bytes), vec!["team.txt"]);
    assert_eq!(
        read_zip_entry_text(&zip_bytes, "team.txt"),
        "team archive payload"
    );

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/archive-download")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [file_id],
            "folder_ids": [],
            "archive_name": "should-fail"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}

#[actix_web::test]
async fn test_personal_archive_compress_task_creates_workspace_file() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    aster_drive::services::config_service::set(
        state.get_ref(),
        "background_task_max_attempts",
        "5",
        1,
    )
    .await
    .expect("background task max attempts config should update");

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "compressor",
        "compressor@example.com",
        "password123"
    );
    let token = login_user!(app, "compressor", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "bundle", "parent_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let bundle_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "docs", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let docs_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "empty", "parent_id": bundle_id }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = multipart_request!(
        &format!("/api/v1/files/upload?folder_id={docs_id}"),
        &token,
        "note.txt",
        "hello from archive compress task",
    );
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    let req = test::TestRequest::post()
        .uri("/api/v1/batch/archive-compress")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({
            "file_ids": [],
            "folder_ids": [bundle_id],
            "archive_name": "bundle-export"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let task_id = body["data"]["id"].as_i64().unwrap();
    assert_eq!(body["data"]["max_attempts"], 5);
    assert_task_steps(
        &body,
        &[
            ("waiting", "active"),
            ("prepare_sources", "pending"),
            ("build_archive", "pending"),
            ("store_result", "pending"),
        ],
    );

    let stats = aster_drive::services::task_service::drain(state.get_ref())
        .await
        .expect("task drain should succeed");
    assert_eq!(stats.succeeded, 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/tasks/{task_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "succeeded");
    assert_task_steps(
        &body,
        &[
            ("waiting", "succeeded"),
            ("prepare_sources", "succeeded"),
            ("build_archive", "succeeded"),
            ("store_result", "succeeded"),
        ],
    );

    let result = read_task_result(&body);
    assert_eq!(result["kind"], "archive_compress");
    let archive_file_id = result["target_file_id"].as_i64().unwrap();
    assert_eq!(result["target_folder_id"], Value::Null);
    assert_eq!(result["target_path"], "/bundle-export.zip");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{archive_file_id}/download"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    let zip_bytes = test::read_body(resp).await;
    assert_eq!(
        zip_entry_names(&zip_bytes),
        vec![
            "bundle/",
            "bundle/docs/",
            "bundle/docs/note.txt",
            "bundle/empty/",
        ]
    );
    assert_eq!(
        read_zip_entry_text(&zip_bytes, "bundle/docs/note.txt"),
        "hello from archive compress task"
    );
}

#[actix_web::test]
async fn test_retry_task_reloads_max_attempts_from_runtime_config() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let owner = aster_drive::db::repository::user_repo::find_by_username(&state.db, "testuser")
        .await
        .expect("owner lookup should succeed")
        .expect("owner should exist");

    aster_drive::services::config_service::set(&state, "background_task_max_attempts", "4", 1)
        .await
        .expect("background task max attempts config should update");

    let now = Utc::now();
    let payload_json = serde_json::to_string(
        &aster_drive::services::task_service::ArchiveCompressTaskPayload {
            file_ids: Vec::new(),
            folder_ids: Vec::new(),
            archive_name: "retry-bundle.zip".to_string(),
            target_folder_id: None,
        },
    )
    .expect("archive payload should serialize");
    let task = background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ArchiveCompress),
            status: Set(BackgroundTaskStatus::Failed),
            creator_user_id: Set(Some(owner.id)),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set("retry-bundle.zip".to_string()),
            payload_json: Set(StoredTaskPayload(payload_json)),
            result_json: Set(None),
            steps_json: Set(None),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(1),
            max_attempts: Set(1),
            next_run_at: Set(now - Duration::seconds(1)),
            processing_token: Set(0),
            processing_started_at: Set(None),
            last_heartbeat_at: Set(None),
            lease_expires_at: Set(None),
            started_at: Set(Some(now - Duration::minutes(2))),
            finished_at: Set(Some(now - Duration::minutes(1))),
            last_error: Set(Some("transient failure".to_string())),
            expires_at: Set(now + Duration::hours(1)),
            created_at: Set(now - Duration::minutes(2)),
            updated_at: Set(now - Duration::minutes(1)),
            ..Default::default()
        },
    )
    .await
    .expect("failed task should be inserted");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/tasks/{}/retry", task.id))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "pending");
    assert_eq!(body["data"]["attempt_count"], 0);
    assert_eq!(body["data"]["max_attempts"], 4);

    let stored = background_task_repo::find_by_id(&state.db, task.id)
        .await
        .expect("retried task should still exist");
    assert_eq!(stored.status, BackgroundTaskStatus::Pending);
    assert_eq!(stored.attempt_count, 0);
    assert_eq!(stored.max_attempts, 4);
}

#[actix_web::test]
async fn test_team_archive_extract_task_creates_team_folder_tree() {
    let state = common::setup().await;
    let db = state.db.clone();
    let mail_sender = state.mail_sender.clone();
    let state = web::Data::new(state);
    let app = test::init_service(
        App::new()
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            .app_data(web::Data::clone(&state))
            .configure(move |cfg| aster_drive::api::configure(cfg, &db)),
    )
    .await;

    register_user!(
        app,
        state.db.clone(),
        mail_sender,
        "extractor",
        "extractor@example.com",
        "password123"
    );
    let token = login_user!(app, "extractor", "password123");

    let req = test::TestRequest::post()
        .uri("/api/v1/teams")
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "Archive Team" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let team_id = body["data"]["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/teams/{team_id}/files/new"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({ "name": "bundle.zip", "folder_id": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let archive_file_id = body["data"]["id"].as_i64().unwrap();

    let archive_bytes = create_zip_bytes(&[
        ("docs/", None),
        ("docs/note.txt", Some("team extract payload".as_bytes())),
        ("empty/", None),
    ]);
    let req = test::TestRequest::put()
        .uri(&format!(
            "/api/v1/teams/{team_id}/files/{archive_file_id}/content"
        ))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .insert_header(("Content-Type", "application/octet-stream"))
        .set_payload(archive_bytes)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/teams/{team_id}/files/{archive_file_id}/extract"
        ))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let task_id = body["data"]["id"].as_i64().unwrap();
    assert_task_steps(
        &body,
        &[
            ("waiting", "active"),
            ("download_source", "pending"),
            ("extract_archive", "pending"),
            ("import_result", "pending"),
        ],
    );

    let stats = aster_drive::services::task_service::drain(state.get_ref())
        .await
        .expect("task drain should succeed");
    assert_eq!(stats.succeeded, 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/teams/{team_id}/tasks/{task_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["status"], "succeeded");
    assert_task_steps(
        &body,
        &[
            ("waiting", "succeeded"),
            ("download_source", "succeeded"),
            ("extract_archive", "succeeded"),
            ("import_result", "succeeded"),
        ],
    );

    let result = read_task_result(&body);
    assert_eq!(result["kind"], "archive_extract");
    let extracted_root_id = result["target_folder_id"].as_i64().unwrap();
    assert_eq!(result["target_folder_name"], "bundle");
    assert_eq!(result["target_path"], "/bundle");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/teams/{team_id}/folders/{extracted_root_id}"
        ))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let folders = body["data"]["folders"].as_array().unwrap();
    assert_eq!(folders.len(), 2);

    let docs_folder = folders
        .iter()
        .find(|folder| folder["name"] == "docs")
        .expect("docs folder should exist");
    let docs_folder_id = docs_folder["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/teams/{team_id}/folders/{docs_folder_id}"))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: Value = test::read_body_json(resp).await;
    let files = body["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "note.txt");
    let note_file_id = files[0]["id"].as_i64().unwrap();

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/teams/{team_id}/files/{note_file_id}/download"
        ))
        .insert_header(("Cookie", common::access_cookie_header(&token)))
        .insert_header(common::csrf_header_for(&token))
        .to_request();
    let resp = assert_response_status(
        test::call_service(&app, req).await,
        actix_web::http::StatusCode::OK,
    )
    .await;
    let file_bytes = test::read_body(resp).await;
    assert_eq!(String::from_utf8_lossy(&file_bytes), "team extract payload");
}
