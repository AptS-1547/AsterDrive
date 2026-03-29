//! 上传集成测试（分片 + presigned）

#[macro_use]
mod common;

use actix_web::test;
use serde_json::Value;
use tokio::task::JoinSet;

const TEST_CHUNK_SIZE: usize = 5_242_880;

async fn create_upload_session(
    state: &aster_drive::runtime::AppState,
    user_id: i64,
    upload_id: &str,
    status: aster_drive::types::UploadSessionStatus,
    expires_at: chrono::DateTime<chrono::Utc>,
    total_chunks: i32,
    received_count: i32,
    s3_temp_key: Option<&str>,
    s3_multipart_id: Option<&str>,
    file_id: Option<i64>,
) {
    use aster_drive::db::repository::{policy_repo, upload_session_repo};
    use sea_orm::Set;

    let policy = policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist in test setup");
    let now = chrono::Utc::now();
    upload_session_repo::create(
        &state.db,
        aster_drive::entities::upload_session::ActiveModel {
            id: Set(upload_id.to_string()),
            user_id: Set(user_id),
            filename: Set("manual-upload.bin".to_string()),
            total_size: Set(10),
            chunk_size: Set(5),
            total_chunks: Set(total_chunks),
            received_count: Set(received_count),
            folder_id: Set(None),
            policy_id: Set(policy.id),
            status: Set(status),
            s3_temp_key: Set(s3_temp_key.map(str::to_string)),
            s3_multipart_id: Set(s3_multipart_id.map(str::to_string)),
            file_id: Set(file_id),
            created_at: Set(now),
            expires_at: Set(expires_at),
            updated_at: Set(now),
        },
    )
    .await
    .unwrap();
}

async fn create_s3_bucket(endpoint: &str, bucket: &str) {
    let credentials =
        aws_credential_types::Credentials::new("rustfsadmin", "rustfsadmin123", None, None, "test");
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .credentials_provider(credentials)
        .endpoint_url(endpoint)
        .force_path_style(true)
        .build();
    let client = aws_sdk_s3::Client::from_conf(config);
    let _ = client.create_bucket().bucket(bucket).send().await;
}

async fn create_s3_default_policy(
    state: &aster_drive::runtime::AppState,
    user_id: i64,
    name: &str,
    endpoint: &str,
    bucket: &str,
    options: &str,
    chunk_size: i64,
) -> aster_drive::entities::storage_policy::Model {
    use aster_drive::db::repository::policy_repo;
    use chrono::Utc;
    use sea_orm::Set;

    let now = Utc::now();
    let policy = policy_repo::create(
        &state.db,
        aster_drive::entities::storage_policy::ActiveModel {
            name: Set(name.to_string()),
            driver_type: Set(aster_drive::types::DriverType::S3),
            endpoint: Set(endpoint.to_string()),
            bucket: Set(bucket.to_string()),
            access_key: Set("rustfsadmin".to_string()),
            secret_key: Set("rustfsadmin123".to_string()),
            base_path: Set("uploads".to_string()),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set(options.to_string()),
            is_default: Set(false),
            chunk_size: Set(chunk_size),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    policy_repo::clear_user_default(&state.db, user_id)
        .await
        .unwrap();
    let _ = policy_repo::create_user_policy(
        &state.db,
        aster_drive::entities::user_storage_policy::ActiveModel {
            user_id: Set(user_id),
            policy_id: Set(policy.id),
            is_default: Set(true),
            quota_bytes: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    policy
}

fn build_multipart_payload(filename: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("----AsterTestBoundary{}", uuid::Uuid::new_v4().simple());
    let mut payload = Vec::new();
    payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    payload.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    payload.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    payload.extend_from_slice(data);
    payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, payload)
}

#[actix_web::test]
async fn test_chunked_upload_flow() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 1. 初始化分片上传（10KB 文件，chunk_size=5MB → 直传模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "chunked.txt",
            "total_size": 10240
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    // 小文件可能返回 direct 模式
    let mode = body["data"]["mode"].as_str().unwrap();
    assert!(
        mode == "direct" || mode == "chunked",
        "mode should be direct or chunked, got {mode}"
    );

    if mode == "chunked" {
        let upload_id = body["data"]["upload_id"].as_str().unwrap().to_string();
        let total_chunks = body["data"]["total_chunks"].as_i64().unwrap();

        // 2. 上传分片
        for i in 0..total_chunks {
            let chunk_data = vec![b'A'; 5120]; // 5KB per chunk
            let req = test::TestRequest::put()
                .uri(&format!("/api/v1/files/upload/{upload_id}/{i}"))
                .insert_header(("Cookie", format!("aster_access={token}")))
                .insert_header(("Content-Type", "application/octet-stream"))
                .set_payload(chunk_data)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200, "chunk {i} upload failed");
        }

        // 3. 查看进度
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 4. 完成上传
        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/files/upload/{upload_id}/complete"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["data"]["name"], "chunked.txt");
    }
}

#[actix_web::test]
async fn test_update_storage_used_is_atomic_under_concurrency() {
    use aster_drive::db::repository::user_repo;
    use aster_drive::services::auth_service;

    let state = common::setup().await;
    let user = auth_service::register(&state, "quotauser", "quota@test.com", "password123")
        .await
        .unwrap();

    let mut tasks = JoinSet::new();
    for _ in 0..32 {
        let db = state.db.clone();
        let user_id = user.id;
        tasks.spawn(async move { user_repo::update_storage_used(&db, user_id, 1).await });
    }

    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }

    let updated = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(updated.storage_used, 32);

    let mut tasks = JoinSet::new();
    for _ in 0..40 {
        let db = state.db.clone();
        let user_id = user.id;
        tasks.spawn(async move { user_repo::update_storage_used(&db, user_id, -1).await });
    }

    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }

    let updated = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(
        updated.storage_used, 0,
        "storage_used should not go below zero"
    );
}

#[actix_web::test]
async fn test_chunked_upload_streaming_assembly_preserves_content() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "streamuser", "stream@test.com", "password123")
        .await
        .unwrap();

    let init = upload_service::init_upload(&state, user.id, "streamed.txt", 10_485_760, None, None)
        .await
        .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);

    let upload_id = init.upload_id.unwrap();
    let chunk0 = vec![b'A'; TEST_CHUNK_SIZE];
    let chunk1 = vec![b'B'; TEST_CHUNK_SIZE];

    let resp0 = upload_service::upload_chunk(&state, &upload_id, 0, user.id, &chunk0)
        .await
        .unwrap();
    assert_eq!(resp0.received_count, 1);
    let resp1 = upload_service::upload_chunk(&state, &upload_id, 1, user.id, &chunk1)
        .await
        .unwrap();
    assert_eq!(resp1.received_count, 2);

    let file = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();
    assert_eq!(file.name, "streamed.txt");

    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let policy = aster_drive::db::repository::policy_repo::find_by_id(&state.db, blob.policy_id)
        .await
        .unwrap();
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let stored = driver.get(&blob.storage_path).await.unwrap();

    assert_eq!(stored, [chunk0.as_slice(), chunk1.as_slice()].concat());
    assert_eq!(blob.size, stored.len() as i64);
}

#[actix_web::test]
async fn test_direct_and_chunked_upload_produce_same_blob_for_same_content() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::{auth_service, file_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "compareuser", "compare@test.com", "password123")
        .await
        .unwrap();

    let pattern = b"same content across direct and chunked upload paths\n";
    let content = pattern.repeat((10_485_760 / pattern.len()) + 1);
    let content = &content[..10_485_760];
    let temp_path = format!("{}/{}", aster_drive::utils::TEMP_DIR, uuid::Uuid::new_v4());
    tokio::fs::create_dir_all(aster_drive::utils::TEMP_DIR)
        .await
        .unwrap();
    tokio::fs::write(&temp_path, content).await.unwrap();

    let direct_file = file_service::store_from_temp(
        &state,
        user.id,
        None,
        "same-direct.txt",
        &temp_path,
        content.len() as i64,
        None,
        false,
    )
    .await
    .unwrap();

    let init = upload_service::init_upload(
        &state,
        user.id,
        "same-chunked.txt",
        content.len() as i64,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);

    let upload_id = init.upload_id.unwrap();
    let total_chunks = init.total_chunks.unwrap();
    let chunk_size = init.chunk_size.unwrap() as usize;
    for chunk_number in 0..total_chunks {
        let start = chunk_number as usize * chunk_size;
        let end = ((chunk_number as usize + 1) * chunk_size).min(content.len());
        let chunk = &content[start..end];
        upload_service::upload_chunk(&state, &upload_id, chunk_number, user.id, chunk)
            .await
            .unwrap();
    }
    let chunked_file = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();

    let direct_blob = file_repo::find_blob_by_id(&state.db, direct_file.blob_id)
        .await
        .unwrap();
    let chunked_blob = file_repo::find_blob_by_id(&state.db, chunked_file.blob_id)
        .await
        .unwrap();

    assert_eq!(direct_blob.id, chunked_blob.id);
    assert_eq!(direct_blob.hash, chunked_blob.hash);
    assert_eq!(direct_blob.size, chunked_blob.size);
    assert_eq!(direct_blob.ref_count, 2);

    let _ = tokio::fs::remove_file(&temp_path).await;
}

#[actix_web::test]
async fn test_chunked_upload_cancel() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    // 初始化大文件上传（强制 chunked 模式）
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "big.bin",
            "total_size": 10_485_760  // 10MB → 超过 chunk_size(5MB) → chunked
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;

    if let Some(upload_id) = body["data"]["upload_id"].as_str() {
        // 取消上传
        let req = test::TestRequest::delete()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // 再查进度应该 404
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/files/upload/{upload_id}"))
            .insert_header(("Cookie", format!("aster_access={token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status() == 404 || resp.status() == 410);
    }
}

/// 测试 init_upload：Local 策略下不返回 presigned
#[actix_web::test]
async fn test_init_upload_local_never_presigned() {
    let state = common::setup().await;
    let app = create_test_app!(state);
    let (token, _) = register_and_login!(app);

    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload/init")
        .insert_header(("Cookie", format!("aster_access={token}")))
        .set_json(serde_json::json!({
            "filename": "test.bin",
            "total_size": 1024
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let mode = body["data"]["mode"].as_str().unwrap();
    assert_ne!(
        mode, "presigned",
        "local storage should never use presigned"
    );
    assert!(body["data"]["presigned_url"].is_null());
}

/// 并发上传同一分片不会导致 received_count 多算（TOCTOU 修复验证）
#[tokio::test]
async fn test_concurrent_chunk_upload_idempotent() {
    use aster_drive::services::{auth_service, upload_service};
    use std::sync::Arc;

    let state = Arc::new(common::setup().await);
    let user = auth_service::register(&state, "testuser", "test@example.com", "password123")
        .await
        .unwrap();
    let upload_id = format!("concurrent-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Uploading,
        chrono::Utc::now() + chrono::Duration::hours(1),
        2,
        0,
        None,
        None,
        None,
    )
    .await;
    tokio::fs::create_dir_all(format!("data/.uploads/{upload_id}"))
        .await
        .unwrap();

    let chunk_data = b"12345".to_vec();
    let state1 = Arc::clone(&state);
    let state2 = Arc::clone(&state);
    let upload_id1 = upload_id.clone();
    let upload_id2 = upload_id.clone();
    let chunk1 = chunk_data.clone();
    let chunk2 = chunk_data.clone();

    let (first, second) = tokio::join!(
        upload_service::upload_chunk(&state1, &upload_id1, 0, user.id, &chunk1),
        upload_service::upload_chunk(&state2, &upload_id2, 0, user.id, &chunk2),
    );

    let first = first.unwrap();
    let second = second.unwrap();
    let final_progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();

    assert!(
        [first.received_count, second.received_count].contains(&1),
        "at least one concurrent upload should observe received_count=1"
    );
    assert_eq!(
        final_progress.received_count, 1,
        "duplicate concurrent chunk upload should not increment count twice"
    );

    let third = upload_service::upload_chunk(&state, &upload_id, 0, user.id, &chunk_data)
        .await
        .unwrap();
    assert_eq!(
        third.received_count, 1,
        "sequential duplicate should remain idempotent after concurrent uploads"
    );
}

#[tokio::test]
async fn test_upload_session_part_upsert_updates_existing_row_without_duplicates() {
    use aster_drive::db::repository::{upload_session_part_repo, upload_session_repo};
    use aster_drive::services::auth_service;

    let state = common::setup().await;
    let user = auth_service::register(&state, "partuser", "part@test.com", "password123")
        .await
        .unwrap();
    let upload_id = format!("parts-{}", uuid::Uuid::new_v4());

    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Uploading,
        chrono::Utc::now() + chrono::Duration::hours(1),
        2,
        0,
        None,
        None,
        None,
    )
    .await;

    let first = upload_session_part_repo::upsert_part(&state.db, &upload_id, 1, "etag-1", 5)
        .await
        .unwrap();
    assert!(first.inserted);
    assert_eq!(first.model.etag, "etag-1");
    assert_eq!(first.model.size, 5);

    let second = upload_session_part_repo::upsert_part(&state.db, &upload_id, 1, "etag-2", 7)
        .await
        .unwrap();
    assert!(!second.inserted);
    assert_eq!(second.model.etag, "etag-2");
    assert_eq!(second.model.size, 7);

    let parts = upload_session_part_repo::list_by_upload(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].part_number, 1);
    assert_eq!(parts[0].etag, "etag-2");
    assert_eq!(parts[0].size, 7);

    let session = upload_session_repo::find_by_id(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(session.received_count, 0);
}

#[actix_web::test]
async fn test_upload_chunk_rejects_wrong_chunk_size() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "sizeuser", "size@test.com", "password123")
        .await
        .unwrap();

    let init =
        upload_service::init_upload(&state, user.id, "size-check.bin", 10_485_760, None, None)
            .await
            .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);

    let upload_id = init.upload_id.unwrap();
    let err = match upload_service::upload_chunk(&state, &upload_id, 0, user.id, b"short").await {
        Ok(_) => panic!("wrong-sized chunk upload should fail"),
        Err(err) => err,
    };
    assert_eq!(err.code(), "E056");
    assert!(err.message().contains("size mismatch"));

    let progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(progress.received_count, 0);
    assert!(progress.chunks_on_disk.is_empty());
}

#[actix_web::test]
async fn test_complete_upload_is_idempotent_after_completion() {
    use aster_drive::db::repository::upload_session_repo;
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "idemuser", "idem@test.com", "password123")
        .await
        .unwrap();

    let init =
        upload_service::init_upload(&state, user.id, "idempotent.txt", 10_485_760, None, None)
            .await
            .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);
    assert_eq!(init.total_chunks, Some(2));

    let upload_id = init.upload_id.unwrap();
    let chunk0 = vec![b'A'; TEST_CHUNK_SIZE];
    let chunk1 = vec![b'B'; TEST_CHUNK_SIZE];
    upload_service::upload_chunk(&state, &upload_id, 0, user.id, &chunk0)
        .await
        .unwrap();
    upload_service::upload_chunk(&state, &upload_id, 1, user.id, &chunk1)
        .await
        .unwrap();

    let first = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();
    let second = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();

    assert_eq!(second.id, first.id);
    assert_eq!(second.blob_id, first.blob_id);
    assert_eq!(second.name, "idempotent.txt");

    let session = upload_session_repo::find_by_id(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(
        session.status,
        aster_drive::types::UploadSessionStatus::Completed
    );
    assert_eq!(session.file_id, Some(first.id));
}

#[actix_web::test]
async fn test_complete_upload_marks_session_failed_after_assembly_error() {
    use aster_drive::db::repository::upload_session_repo;
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "faileduser", "failed@test.com", "password123")
        .await
        .unwrap();

    let init = upload_service::init_upload(&state, user.id, "broken.txt", 10_485_760, None, None)
        .await
        .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);

    let upload_id = init.upload_id.unwrap();
    let chunk0 = vec![b'A'; TEST_CHUNK_SIZE];
    let chunk1 = vec![b'B'; TEST_CHUNK_SIZE];
    upload_service::upload_chunk(&state, &upload_id, 0, user.id, &chunk0)
        .await
        .unwrap();
    upload_service::upload_chunk(&state, &upload_id, 1, user.id, &chunk1)
        .await
        .unwrap();

    tokio::fs::remove_file(format!("data/.uploads/{upload_id}/chunk_1"))
        .await
        .unwrap();

    let err = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "E057");
    assert!(err.message().contains("open chunk 1"));

    let session = upload_session_repo::find_by_id(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(
        session.status,
        aster_drive::types::UploadSessionStatus::Failed
    );
    assert_eq!(session.file_id, None);

    let retry_err = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap_err();
    assert_eq!(retry_err.code(), "E057");
    assert!(retry_err.message().contains("previously"));
}

#[actix_web::test]
async fn test_upload_service_complete_rejects_assembling_session() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "assemblinguser",
        "assembling@test.com",
        "password123",
    )
    .await
    .unwrap();
    let upload_id = format!("assembling-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Assembling,
        chrono::Utc::now() + chrono::Duration::hours(1),
        2,
        2,
        None,
        None,
        None,
    )
    .await;

    let err = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap_err();
    assert!(err.message().contains("being processed"));
}

#[actix_web::test]
async fn test_upload_service_complete_completed_without_file_id_returns_refresh_hint() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "completeuser", "complete@test.com", "password123")
        .await
        .unwrap();
    let upload_id = format!("completed-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Completed,
        chrono::Utc::now() + chrono::Duration::hours(1),
        0,
        0,
        None,
        None,
        None,
    )
    .await;

    let err = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap_err();
    assert!(err.message().contains("file_id not found"));
    assert!(err.message().contains("refresh"));
}

#[actix_web::test]
async fn test_upload_service_complete_presigned_multipart_requires_parts() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(
        &state,
        "partsuser",
        "multipartparts@test.com",
        "password123",
    )
    .await
    .unwrap();
    let upload_id = format!("multipart-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Presigned,
        chrono::Utc::now() + chrono::Duration::hours(1),
        2,
        0,
        Some("files/temp-key"),
        Some("multipart-id"),
        None,
    )
    .await;

    let err = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap_err();
    assert!(err.message().contains("parts required"));
}

#[actix_web::test]
async fn test_upload_service_get_progress_scans_and_sorts_local_chunks() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "progressuser", "progress@test.com", "password123")
        .await
        .unwrap();
    let upload_id = format!("progress-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Uploading,
        chrono::Utc::now() + chrono::Duration::hours(1),
        3,
        2,
        None,
        None,
        None,
    )
    .await;

    let temp_dir = format!("data/.uploads/{upload_id}");
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    tokio::fs::write(format!("{temp_dir}/chunk_2"), b"two")
        .await
        .unwrap();
    tokio::fs::write(format!("{temp_dir}/chunk_0"), b"zero")
        .await
        .unwrap();
    tokio::fs::write(format!("{temp_dir}/chunk_bad"), b"ignored")
        .await
        .unwrap();
    tokio::fs::write(format!("{temp_dir}/notes.txt"), b"ignored")
        .await
        .unwrap();

    let progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(progress.upload_id, upload_id);
    assert_eq!(progress.received_count, 2);
    assert_eq!(progress.total_chunks, 3);
    assert_eq!(progress.chunks_on_disk, vec![0, 2]);
}

#[actix_web::test]
async fn test_upload_service_presign_parts_rejects_non_multipart_session() {
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "presignuser", "presign@test.com", "password123")
        .await
        .unwrap();
    let upload_id = format!("presign-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &upload_id,
        aster_drive::types::UploadSessionStatus::Presigned,
        chrono::Utc::now() + chrono::Duration::hours(1),
        1,
        0,
        Some("files/temp-key"),
        None,
        None,
    )
    .await;

    let err = upload_service::presign_parts(&state, &upload_id, user.id, vec![1])
        .await
        .unwrap_err();
    assert!(err.message().contains("not a multipart upload session"));
}

#[actix_web::test]
async fn test_upload_service_cleanup_expired_removes_local_sessions_only() {
    use aster_drive::db::repository::upload_session_repo;
    use aster_drive::services::{auth_service, upload_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "cleanupuser", "cleanup@test.com", "password123")
        .await
        .unwrap();

    let expired_id = format!("expired-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &expired_id,
        aster_drive::types::UploadSessionStatus::Uploading,
        chrono::Utc::now() - chrono::Duration::minutes(5),
        2,
        1,
        None,
        None,
        None,
    )
    .await;

    let completed_id = format!("completed-{}", uuid::Uuid::new_v4());
    create_upload_session(
        &state,
        user.id,
        &completed_id,
        aster_drive::types::UploadSessionStatus::Completed,
        chrono::Utc::now() - chrono::Duration::minutes(5),
        0,
        0,
        None,
        None,
        Some(123),
    )
    .await;

    let expired_dir = format!("data/.uploads/{expired_id}");
    tokio::fs::create_dir_all(&expired_dir).await.unwrap();
    tokio::fs::write(format!("{expired_dir}/chunk_0"), b"temp")
        .await
        .unwrap();

    let cleaned = upload_service::cleanup_expired(&state).await.unwrap();
    assert_eq!(cleaned, 1);
    assert!(
        upload_session_repo::find_by_id(&state.db, &expired_id)
            .await
            .is_err()
    );
    assert!(
        upload_session_repo::find_by_id(&state.db, &completed_id)
            .await
            .is_ok()
    );
    assert!(
        !std::path::Path::new(&expired_dir).exists(),
        "expired temp dir should be removed"
    );
}

/// S3 presigned upload 端到端测试（需要 testcontainers + rustfs）
#[tokio::test]
async fn test_presigned_upload_s3_e2e() {
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    // 启动 rustfs 容器
    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-presigned";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 创建 bucket
    {
        let credentials = aws_credential_types::Credentials::new(
            "rustfsadmin",
            "rustfsadmin123",
            None,
            None,
            "test",
        );
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(credentials)
            .endpoint_url(&endpoint)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);
        let _ = client.create_bucket().bucket(bucket).send().await;
    }

    // 创建 state（内存 SQLite）
    let state = common::setup().await;

    // 创建 S3 策略 + presigned_upload: true
    use chrono::Utc;
    use sea_orm::Set;
    let now = Utc::now();
    let s3_policy = aster_drive::db::repository::policy_repo::create(
        &state.db,
        aster_drive::entities::storage_policy::ActiveModel {
            name: Set("Test S3 Presigned".to_string()),
            driver_type: Set(aster_drive::types::DriverType::S3),
            endpoint: Set(endpoint),
            bucket: Set(bucket.to_string()),
            access_key: Set("rustfsadmin".to_string()),
            secret_key: Set("rustfsadmin123".to_string()),
            base_path: Set("uploads".to_string()),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set(r#"{"presigned_upload":true}"#.to_string()),
            is_default: Set(false),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 注册用户 + 分配 S3 策略为默认
    let user = auth_service::register(&state, "s3user", "s3@test.com", "pass123")
        .await
        .unwrap();
    use aster_drive::db::repository::policy_repo;
    // 清除 register 自动分配的 local default，确保 S3 策略成为唯一 default
    policy_repo::clear_user_default(&state.db, user.id)
        .await
        .unwrap();
    let _ = policy_repo::create_user_policy(
        &state.db,
        aster_drive::entities::user_storage_policy::ActiveModel {
            user_id: Set(user.id),
            policy_id: Set(s3_policy.id),
            is_default: Set(true),
            quota_bytes: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // 1. init_upload → 应返回 presigned 模式
    let data = b"hello presigned world!";
    let init =
        upload_service::init_upload(&state, user.id, "hello.txt", data.len() as i64, None, None)
            .await
            .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Presigned);
    assert!(init.presigned_url.is_some());
    assert!(init.upload_id.is_some());

    let presigned_url = init.presigned_url.unwrap();
    let upload_id = init.upload_id.unwrap();

    // 2. PUT 到 presigned URL（模拟客户端直传）
    let client = reqwest::Client::new();
    let resp = client
        .put(&presigned_url)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "S3 presigned PUT failed: {}",
        resp.status()
    );

    // 3. complete → 服务端 hash + dedup + 建记录
    let file = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();
    assert_eq!(file.name, "hello.txt");

    // 4. 验证文件可通过 driver 读取
    let policy = policy_repo::find_by_id(&state.db, s3_policy.id)
        .await
        .unwrap();
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let blob = aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let got = driver.get(&blob.storage_path).await.unwrap();
    assert_eq!(got, data);

    // 5. 上传相同内容 → S3 presigned 不做 blob 去重（避免回拉 SHA256 抵消直传优势）
    //    每次上传产生独立 blob，各自 ref_count=1
    let init2 =
        upload_service::init_upload(&state, user.id, "hello2.txt", data.len() as i64, None, None)
            .await
            .unwrap();
    let url2 = init2.presigned_url.unwrap();
    let id2 = init2.upload_id.unwrap();
    client
        .put(&url2)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .unwrap();
    let file2 = upload_service::complete_upload(&state, &id2, user.id, None)
        .await
        .unwrap();
    assert_ne!(
        file2.blob_id, file.blob_id,
        "S3 presigned skips dedup — each upload creates its own blob"
    );

    let blob1 = aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let blob2 = aster_drive::db::repository::file_repo::find_blob_by_id(&state.db, file2.blob_id)
        .await
        .unwrap();
    assert_eq!(blob1.ref_count, 1);
    assert_eq!(blob2.ref_count, 1);
}

/// S3 presigned multipart 上传端到端测试：覆盖 presign_parts / progress / complete 排序分支
#[tokio::test]
async fn test_presigned_multipart_upload_s3_e2e() {
    use aster_drive::db::repository::{file_repo, policy_repo};
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-presigned-multipart";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    {
        let credentials = aws_credential_types::Credentials::new(
            "rustfsadmin",
            "rustfsadmin123",
            None,
            None,
            "test",
        );
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(credentials)
            .endpoint_url(&endpoint)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);
        let _ = client.create_bucket().bucket(bucket).send().await;
    }

    let state = common::setup().await;

    use chrono::Utc;
    use sea_orm::Set;
    let now = Utc::now();
    let s3_policy = policy_repo::create(
        &state.db,
        aster_drive::entities::storage_policy::ActiveModel {
            name: Set("Test S3 Multipart".to_string()),
            driver_type: Set(aster_drive::types::DriverType::S3),
            endpoint: Set(endpoint),
            bucket: Set(bucket.to_string()),
            access_key: Set("rustfsadmin".to_string()),
            secret_key: Set("rustfsadmin123".to_string()),
            base_path: Set("uploads".to_string()),
            max_file_size: Set(0),
            allowed_types: Set("[]".to_string()),
            options: Set(r#"{"presigned_upload":true}"#.to_string()),
            is_default: Set(false),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let user = auth_service::register(&state, "s3multipartuser", "s3multipart@test.com", "pass123")
        .await
        .unwrap();

    policy_repo::clear_user_default(&state.db, user.id)
        .await
        .unwrap();
    let _ = policy_repo::create_user_policy(
        &state.db,
        aster_drive::entities::user_storage_policy::ActiveModel {
            user_id: Set(user.id),
            policy_id: Set(s3_policy.id),
            is_default: Set(true),
            quota_bytes: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let mut data = vec![b'A'; 5_242_880];
    data.extend_from_slice(b"multipart tail");
    let (part1, part2) = data.split_at(5_242_880);

    let init = upload_service::init_upload(
        &state,
        user.id,
        "multipart.bin",
        data.len() as i64,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        init.mode,
        aster_drive::types::UploadMode::PresignedMultipart
    );
    assert_eq!(init.total_chunks, Some(2));
    assert!(init.presigned_url.is_none());

    let upload_id = init.upload_id.unwrap();
    let urls = upload_service::presign_parts(&state, &upload_id, user.id, vec![2, 1])
        .await
        .unwrap();
    assert_eq!(urls.len(), 2);

    let client = reqwest::Client::new();
    let resp1 = client
        .put(urls.get(&1).unwrap())
        .header(reqwest::header::CONTENT_LENGTH, part1.len())
        .body(part1.to_vec())
        .send()
        .await
        .unwrap();
    assert!(resp1.status().is_success());
    let etag1 = resp1
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .expect("part 1 etag missing");

    let resp2 = client
        .put(urls.get(&2).unwrap())
        .header(reqwest::header::CONTENT_LENGTH, part2.len())
        .body(part2.to_vec())
        .send()
        .await
        .unwrap();
    assert!(resp2.status().is_success());
    let etag2 = resp2
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .expect("part 2 etag missing");

    let progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(
        progress.status,
        aster_drive::types::UploadSessionStatus::Presigned
    );
    assert_eq!(progress.total_chunks, 2);
    assert_eq!(progress.chunks_on_disk, vec![1, 2]);

    let file = upload_service::complete_upload(
        &state,
        &upload_id,
        user.id,
        Some(vec![(2, etag2), (1, etag1)]),
    )
    .await
    .unwrap();
    assert_eq!(file.name, "multipart.bin");

    let policy = policy_repo::find_by_id(&state.db, s3_policy.id)
        .await
        .unwrap();
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let stored = driver.get(&blob.storage_path).await.unwrap();
    assert_eq!(stored, data);
}

/// S3 relay_stream 小文件直传：走 /files/upload，服务端直接中继到 S3，不做去重
#[tokio::test]
async fn test_relay_stream_direct_upload_s3_e2e() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-relay-direct";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    create_s3_bucket(&endpoint, bucket).await;

    let state = common::setup().await;
    let user = auth_service::register(&state, "relaydirect", "relay-direct@test.com", "pass123")
        .await
        .unwrap();
    let policy = create_s3_default_policy(
        &state,
        user.id,
        "Test S3 Relay Direct",
        &endpoint,
        bucket,
        r#"{"s3_upload_strategy":"relay_stream"}"#,
        5_242_880,
    )
    .await;
    let login = auth_service::login(&state, "relaydirect", "pass123")
        .await
        .unwrap();

    let data = b"hello relay stream!";
    let init =
        upload_service::init_upload(&state, user.id, "relay.txt", data.len() as i64, None, None)
            .await
            .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Direct);

    let db = state.db.clone();
    let driver_registry = state.driver_registry.clone();
    let app = create_test_app!(state);

    let (boundary, payload) = build_multipart_payload("relay.txt", data);
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={}", login.access_token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let (boundary2, payload2) = build_multipart_payload("relay-copy.txt", data);
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={}", login.access_token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary2}"),
        ))
        .set_payload(payload2)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id2 = body["data"]["id"].as_i64().unwrap();

    let file = file_repo::find_by_id(&db, file_id).await.unwrap();
    let file2 = file_repo::find_by_id(&db, file_id2).await.unwrap();
    let blob = file_repo::find_blob_by_id(&db, file.blob_id).await.unwrap();
    let blob2 = file_repo::find_blob_by_id(&db, file2.blob_id)
        .await
        .unwrap();

    assert!(blob.hash.starts_with("s3-"));
    assert!(blob2.hash.starts_with("s3-"));
    assert_eq!(blob.ref_count, 1);
    assert_eq!(blob2.ref_count, 1);
    assert_ne!(blob.id, blob2.id);
    assert_ne!(blob.hash, blob2.hash);
    assert_ne!(blob.storage_path, blob2.storage_path);

    let driver = driver_registry.get_driver(&policy).unwrap();
    let stored = driver.get(&blob.storage_path).await.unwrap();
    let stored2 = driver.get(&blob2.storage_path).await.unwrap();
    assert_eq!(stored, data);
    assert_eq!(stored2, data);
}

/// S3 relay_stream 直传：文件大小刚好等于 multipart part_size 时不应追加空 part
#[tokio::test]
async fn test_relay_stream_direct_upload_s3_exact_part_size_e2e() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-relay-direct-exact-part";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    create_s3_bucket(&endpoint, bucket).await;

    let state = common::setup().await;
    let user = auth_service::register(&state, "relayexact", "relay-exact@test.com", "pass123")
        .await
        .unwrap();
    let policy = create_s3_default_policy(
        &state,
        user.id,
        "Test S3 Relay Direct Exact Part",
        &endpoint,
        bucket,
        r#"{"s3_upload_strategy":"relay_stream"}"#,
        5_242_880,
    )
    .await;
    let login = auth_service::login(&state, "relayexact", "pass123")
        .await
        .unwrap();

    let data = vec![b'Z'; 5_242_880];
    let init = upload_service::init_upload(
        &state,
        user.id,
        "relay-exact.bin",
        data.len() as i64,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Direct);

    let db = state.db.clone();
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let app = create_test_app!(state);

    let (boundary, payload) = build_multipart_payload("relay-exact.bin", &data);
    let req = test::TestRequest::post()
        .uri("/api/v1/files/upload")
        .insert_header(("Cookie", format!("aster_access={}", login.access_token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: Value = test::read_body_json(resp).await;
    let file_id = body["data"]["id"].as_i64().unwrap();

    let file = file_repo::find_by_id(&db, file_id).await.unwrap();
    let blob = file_repo::find_blob_by_id(&db, file.blob_id).await.unwrap();
    let stored = driver.get(&blob.storage_path).await.unwrap();
    assert_eq!(stored, data);
}

/// S3 relay_stream 大文件分片：服务端直接把 chunk 作为 S3 part，中途不落 data/.uploads
#[tokio::test]
async fn test_relay_stream_chunked_upload_s3_e2e() {
    use aster_drive::db::repository::{file_repo, upload_session_part_repo, upload_session_repo};
    use aster_drive::services::{auth_service, upload_service};
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(testcontainers::core::IntoContainerPort::tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", "rustfsadmin")
        .with_env_var("RUSTFS_SECRET_KEY", "rustfsadmin123")
        .start()
        .await
        .expect("failed to start rustfs container");

    let port = container.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://127.0.0.1:{port}");
    let bucket = "test-relay-chunked";

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    create_s3_bucket(&endpoint, bucket).await;

    let state = common::setup().await;
    let user = auth_service::register(&state, "relaychunked", "relay-chunked@test.com", "pass123")
        .await
        .unwrap();
    let policy = create_s3_default_policy(
        &state,
        user.id,
        "Test S3 Relay Chunked",
        &endpoint,
        bucket,
        r#"{"s3_upload_strategy":"relay_stream"}"#,
        1_048_576,
    )
    .await;

    let part1 = vec![b'A'; 5_242_880];
    let part2 = b"relay-stream-tail".to_vec();
    let mut data = part1.clone();
    data.extend_from_slice(&part2);

    let init = upload_service::init_upload(
        &state,
        user.id,
        "relay-multipart.bin",
        data.len() as i64,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(init.mode, aster_drive::types::UploadMode::Chunked);
    assert_eq!(init.chunk_size, Some(5_242_880));
    assert_eq!(init.total_chunks, Some(2));

    let upload_id = init.upload_id.unwrap();
    let relay_temp_dir = format!("data/.uploads/{upload_id}");
    assert!(
        !std::path::Path::new(&relay_temp_dir).exists(),
        "relay_stream should not create local upload temp dir"
    );
    assert!(
        upload_session_part_repo::list_by_upload(&state.db, &upload_id)
            .await
            .unwrap()
            .is_empty()
    );

    let first = upload_service::upload_chunk(&state, &upload_id, 0, user.id, &part1)
        .await
        .unwrap();
    assert_eq!(first.received_count, 1);

    let duplicate = upload_service::upload_chunk(&state, &upload_id, 0, user.id, &part1)
        .await
        .unwrap();
    assert_eq!(
        duplicate.received_count, 1,
        "duplicate relay part upload should be idempotent"
    );

    let progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(progress.chunks_on_disk, vec![0]);

    let parts = upload_session_part_repo::list_by_upload(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].part_number, 1);
    assert_eq!(parts[0].size, part1.len() as i64);
    assert!(!parts[0].etag.is_empty());
    assert!(
        !std::path::Path::new(&relay_temp_dir).exists(),
        "relay_stream should still avoid local temp dirs after part upload"
    );

    let second = upload_service::upload_chunk(&state, &upload_id, 1, user.id, &part2)
        .await
        .unwrap();
    assert_eq!(second.received_count, 2);

    let progress = upload_service::get_progress(&state, &upload_id, user.id)
        .await
        .unwrap();
    assert_eq!(progress.chunks_on_disk, vec![0, 1]);

    let parts = upload_session_part_repo::list_by_upload(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].part_number, 1);
    assert_eq!(parts[1].part_number, 2);
    assert_eq!(parts[0].size, part1.len() as i64);
    assert_eq!(parts[1].size, part2.len() as i64);
    assert!(parts.iter().all(|part| !part.etag.is_empty()));

    let session = upload_session_repo::find_by_id(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(session.received_count, 2);
    assert_eq!(
        session.status,
        aster_drive::types::UploadSessionStatus::Uploading
    );

    let file = upload_service::complete_upload(&state, &upload_id, user.id, None)
        .await
        .unwrap();
    assert_eq!(file.name, "relay-multipart.bin");

    let session = upload_session_repo::find_by_id(&state.db, &upload_id)
        .await
        .unwrap();
    assert_eq!(
        session.status,
        aster_drive::types::UploadSessionStatus::Completed
    );
    assert_eq!(session.file_id, Some(file.id));

    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    assert_eq!(blob.hash, format!("s3-{upload_id}"));
    assert_eq!(blob.storage_path, format!("files/{upload_id}"));
    assert_eq!(blob.ref_count, 1);

    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let stored = driver.get(&blob.storage_path).await.unwrap();
    assert_eq!(stored, data);
    assert!(
        !std::path::Path::new(&relay_temp_dir).exists(),
        "relay_stream multipart should never use local chunk temp dir"
    );
}
