//! Storage policy blob migration integration tests.

#[macro_use]
mod common;

use actix_web::test;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::Value;

use aster_drive::db::repository::{
    background_task_repo, file_repo, policy_repo, storage_migration_checkpoint_repo,
};
use aster_drive::entities::{file, file_blob, file_version, storage_policy};
use aster_drive::runtime::PrimaryAppState;
use aster_drive::services::task_service;
use aster_drive::types::{
    BackgroundTaskStatus, DriverType, FileCategory, StoredStoragePolicyAllowedTypes,
    StoredStoragePolicyOptions,
};

async fn create_local_policy(state: &PrimaryAppState, name: &str) -> storage_policy::Model {
    let now = Utc::now();
    let base_path = format!("{}/policy-{name}", state.config.server.temp_dir);
    tokio::fs::create_dir_all(&base_path)
        .await
        .expect("policy test dir should be created");
    policy_repo::create(
        state.writer_db(),
        storage_policy::ActiveModel {
            name: Set(name.to_string()),
            driver_type: Set(DriverType::Local),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set(base_path),
            max_file_size: Set(0),
            allowed_types: Set(StoredStoragePolicyAllowedTypes::empty()),
            options: Set(StoredStoragePolicyOptions::empty()),
            is_default: Set(false),
            chunk_size: Set(5_242_880),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .expect("local policy should insert")
}

async fn create_blob_with_object(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    bytes: &[u8],
    ref_count: i32,
) -> file_blob::Model {
    let hash = aster_drive::utils::hash::sha256_hex(bytes);
    let storage_path = aster_drive::utils::storage_path_from_blob_key(&hash);
    let full_path = std::path::Path::new(&policy.base_path).join(&storage_path);
    tokio::fs::create_dir_all(full_path.parent().expect("blob path should have parent"))
        .await
        .expect("blob parent should be created");
    tokio::fs::write(&full_path, bytes)
        .await
        .expect("blob object should be written");
    let now = Utc::now();
    file_blob::ActiveModel {
        hash: Set(hash),
        size: Set(i64::try_from(bytes.len()).expect("test bytes len should fit i64")),
        policy_id: Set(policy.id),
        storage_path: Set(storage_path),
        thumbnail_path: Set(Some("old-thumb".to_string())),
        thumbnail_processor: Set(Some("old-processor".to_string())),
        thumbnail_version: Set(Some("old-version".to_string())),
        ref_count: Set(ref_count),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(state.writer_db())
    .await
    .expect("blob row should insert")
}

async fn create_opaque_blob_with_object(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    blob_key: &str,
    bytes: &[u8],
    ref_count: i32,
) -> file_blob::Model {
    let storage_path = aster_drive::utils::storage_path_from_blob_key(blob_key);
    let full_path = std::path::Path::new(&policy.base_path).join(&storage_path);
    tokio::fs::create_dir_all(full_path.parent().expect("blob path should have parent"))
        .await
        .expect("blob parent should be created");
    tokio::fs::write(&full_path, bytes)
        .await
        .expect("blob object should be written");
    let now = Utc::now();
    file_blob::ActiveModel {
        hash: Set(blob_key.to_string()),
        size: Set(i64::try_from(bytes.len()).expect("test bytes len should fit i64")),
        policy_id: Set(policy.id),
        storage_path: Set(storage_path),
        thumbnail_path: Set(None),
        thumbnail_processor: Set(None),
        thumbnail_version: Set(None),
        ref_count: Set(ref_count),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(state.writer_db())
    .await
    .expect("opaque blob row should insert")
}

async fn create_file_for_blob(state: &PrimaryAppState, blob_id: i64, name: &str) -> file::Model {
    let now = Utc::now();
    file::ActiveModel {
        name: Set(name.to_string()),
        folder_id: Set(None),
        team_id: Set(None),
        blob_id: Set(blob_id),
        size: Set(1),
        owner_user_id: Set(None),
        created_by_user_id: Set(None),
        created_by_username: Set("tester".to_string()),
        mime_type: Set("text/plain".to_string()),
        extension: Set("txt".to_string()),
        compound_extension: Set(None),
        file_category: Set(FileCategory::Document),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        is_locked: Set(false),
        ..Default::default()
    }
    .insert(state.writer_db())
    .await
    .expect("file row should insert")
}

async fn create_version_for_blob(
    state: &PrimaryAppState,
    file_id: i64,
    blob_id: i64,
    version: i32,
) -> file_version::Model {
    file_version::ActiveModel {
        file_id: Set(file_id),
        blob_id: Set(blob_id),
        version: Set(version),
        size: Set(1),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(state.writer_db())
    .await
    .expect("file version row should insert")
}

async fn create_migration_task_via_api(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: &str,
    source_policy_id: i64,
    target_policy_id: i64,
    delete_source_after_success: bool,
) -> Value {
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/storage-migrations")
        .insert_header(("Cookie", common::access_cookie_header(token)))
        .insert_header(common::csrf_header_for(token))
        .set_json(serde_json::json!({
            "source_policy_id": source_policy_id,
            "target_policy_id": target_policy_id,
            "delete_source_after_success": delete_source_after_success,
        }))
        .to_request();
    let resp = test::call_service(app, req).await;
    test::read_body_json(resp).await
}

#[actix_web::test]
async fn test_storage_migration_api_creates_task_and_checkpoint() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-create").await;
    let target = create_local_policy(&state, "target-create").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;

    assert_eq!(body["code"], 0);
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    assert_eq!(body["data"]["kind"], "storage_policy_migration");
    assert_eq!(
        body["data"]["payload"]["source_policy_id"].as_i64(),
        Some(source.id)
    );
    assert_eq!(
        body["data"]["payload"]["target_policy_id"].as_i64(),
        Some(target.id)
    );

    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.source_policy_id, source.id);
    assert_eq!(checkpoint.target_policy_id, target.id);
    assert_eq!(checkpoint.last_processed_blob_id, 0);
}

#[actix_web::test]
async fn test_storage_migration_api_rejects_source_deletion_flag() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-delete-flag").await;
    let target = create_local_policy(&state, "target-delete-flag").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, true).await;

    assert_ne!(body["code"], 0);
    assert!(
        body["msg"]
            .as_str()
            .expect("error message should exist")
            .contains("delete_source_after_success")
    );
}

#[actix_web::test]
async fn test_storage_migration_moves_blob_to_empty_target_policy() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-move").await;
    let target = create_local_policy(&state, "target-move").await;
    let blob = create_blob_with_object(&state, &source, b"move-me", 1).await;
    create_file_for_blob(&state, blob.id, "move.txt").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    let stats = task_service::drain(&state)
        .await
        .expect("migration task should drain");
    assert_eq!(stats.succeeded, 1);

    let task = background_task_repo::find_by_id(state.writer_db(), task_id)
        .await
        .expect("task should exist");
    assert_eq!(task.status, BackgroundTaskStatus::Succeeded);
    let migrated = file_repo::find_blob_by_id(state.writer_db(), blob.id)
        .await
        .expect("blob should still exist");
    assert_eq!(migrated.policy_id, target.id);
    assert_eq!(
        migrated.storage_path,
        aster_drive::utils::storage_path_from_blob_key(&blob.hash)
    );
    assert!(migrated.thumbnail_path.is_none());
    assert!(
        std::path::Path::new(&target.base_path)
            .join(&migrated.storage_path)
            .exists()
    );
    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.scanned_blobs, 1);
    assert_eq!(checkpoint.migrated_blobs, 1);
    assert_eq!(checkpoint.merged_blobs, 0);
    assert_eq!(checkpoint.migrated_bytes, 7);
}

#[actix_web::test]
async fn test_storage_migration_moves_opaque_local_blob_key_without_content_hash_mismatch() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-opaque").await;
    let target = create_local_policy(&state, "target-opaque").await;
    let blob = create_opaque_blob_with_object(
        &state,
        &source,
        "8a7ab9852bc34e98ac1fd29ddd94365b",
        b"opaque blob bytes",
        1,
    )
    .await;
    create_file_for_blob(&state, blob.id, "opaque.txt").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let stats = task_service::drain(&state)
        .await
        .expect("opaque migration task should drain");
    assert_eq!(stats.succeeded, 1);

    let migrated = file_repo::find_blob_by_id(state.writer_db(), blob.id)
        .await
        .expect("blob should still exist");
    assert_eq!(migrated.hash, blob.hash);
    assert_eq!(migrated.policy_id, target.id);
    let target_object = tokio::fs::read(
        std::path::Path::new(&target.base_path)
            .join(aster_drive::utils::storage_path_from_blob_key(&blob.hash)),
    )
    .await
    .expect("target object should exist");
    assert_eq!(target_object, b"opaque blob bytes");

    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    let task = background_task_repo::find_by_id(state.writer_db(), task_id)
        .await
        .expect("task should exist");
    assert_eq!(task.status, BackgroundTaskStatus::Succeeded);
}

#[actix_web::test]
async fn test_storage_migration_merges_when_target_blob_already_exists() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-merge").await;
    let target = create_local_policy(&state, "target-merge").await;
    let source_blob = create_blob_with_object(&state, &source, b"same-bytes", 2).await;
    let target_blob = create_blob_with_object(&state, &target, b"same-bytes", 1).await;
    let active_file = create_file_for_blob(&state, source_blob.id, "active.txt").await;
    create_version_for_blob(&state, active_file.id, source_blob.id, 1).await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    let stats = task_service::drain(&state)
        .await
        .expect("migration task should drain");
    assert_eq!(stats.succeeded, 1);

    assert!(
        file_repo::find_blob_by_id(state.writer_db(), source_blob.id)
            .await
            .is_err(),
        "old source blob row should be deleted after merge"
    );
    let merged_target = file_repo::find_blob_by_id(state.writer_db(), target_blob.id)
        .await
        .expect("target blob should exist");
    assert_eq!(merged_target.ref_count, 3);
    let updated_file = file::Entity::find_by_id(active_file.id)
        .one(state.writer_db())
        .await
        .expect("file query should succeed")
        .expect("file should exist");
    assert_eq!(updated_file.blob_id, target_blob.id);
    let updated_version = file_version::Entity::find()
        .filter(file_version::Column::FileId.eq(active_file.id))
        .one(state.writer_db())
        .await
        .expect("version query should succeed")
        .expect("version should exist");
    assert_eq!(updated_version.blob_id, target_blob.id);
    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.scanned_blobs, 1);
    assert_eq!(checkpoint.migrated_blobs, 0);
    assert_eq!(checkpoint.merged_blobs, 1);
    assert_eq!(checkpoint.migrated_bytes, 10);
}

#[actix_web::test]
async fn test_storage_migration_empty_source_succeeds_with_zero_counts() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-empty").await;
    let target = create_local_policy(&state, "target-empty").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    let stats = task_service::drain(&state)
        .await
        .expect("empty migration task should drain");
    assert_eq!(stats.succeeded, 1);

    let task = background_task_repo::find_by_id(state.writer_db(), task_id)
        .await
        .expect("task should exist");
    assert_eq!(task.status, BackgroundTaskStatus::Succeeded);
    assert_eq!(task.progress_current, 0);
    assert_eq!(task.progress_total, 0);

    let result: aster_drive::services::task_service::StoragePolicyMigrationTaskResult =
        serde_json::from_str(
            task.result_json
                .as_ref()
                .map(AsRef::as_ref)
                .expect("successful migration should store result"),
        )
        .expect("result json should parse");
    assert_eq!(result.scanned_blobs, 0);
    assert_eq!(result.migrated_blobs, 0);
    assert_eq!(result.merged_blobs, 0);
    assert_eq!(result.skipped_blobs, 0);
    assert_eq!(result.failed_blobs, 0);
    assert_eq!(result.migrated_bytes, 0);

    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.stage, "complete");
    assert_eq!(checkpoint.last_processed_blob_id, 0);
    assert_eq!(checkpoint.scanned_blobs, 0);
    assert_eq!(checkpoint.migrated_blobs, 0);
    assert_eq!(checkpoint.merged_blobs, 0);
    assert_eq!(checkpoint.skipped_blobs, 0);
    assert_eq!(checkpoint.failed_blobs, 0);
}

#[actix_web::test]
async fn test_storage_migration_cleans_target_object_when_verification_fails() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-cleanup").await;
    let target = create_local_policy(&state, "target-cleanup").await;
    let blob = create_blob_with_object(&state, &source, b"cleanup-me", 1).await;
    create_file_for_blob(&state, blob.id, "cleanup.txt").await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");
    let source_full_path = std::path::Path::new(&source.base_path).join(&blob.storage_path);
    tokio::fs::write(&source_full_path, b"bad-data!!")
        .await
        .expect("source object should be tampered before migration starts");

    let stats = task_service::drain(&state)
        .await
        .expect("failed migration task should drain");
    assert_eq!(stats.failed, 1);

    let task = background_task_repo::find_by_id(state.writer_db(), task_id)
        .await
        .expect("task should exist");
    assert_eq!(task.status, BackgroundTaskStatus::Failed);
    assert!(
        task.last_error
            .as_deref()
            .expect("failed task should store last_error")
            .contains("copied blob hash mismatch")
    );

    let source_blob = file_repo::find_blob_by_id(state.writer_db(), blob.id)
        .await
        .expect("source blob row should remain");
    assert_eq!(source_blob.policy_id, source.id);
    let target_driver = state
        .driver_registry
        .get_driver(&target)
        .expect("target driver should exist");
    let target_path = aster_drive::utils::storage_path_from_blob_key(&blob.hash);
    assert!(
        !target_driver
            .exists(&target_path)
            .await
            .expect("target existence check should succeed"),
        "failed migration should cleanup the target object"
    );

    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.stage, "migrate_blobs");
    assert_eq!(checkpoint.last_processed_blob_id, 0);
    assert_eq!(checkpoint.scanned_blobs, 0);
    assert_eq!(checkpoint.migrated_blobs, 0);
}

#[actix_web::test]
async fn test_storage_migration_fails_when_policy_changes_after_task_creation() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-changed").await;
    let target = create_local_policy(&state, "target-changed").await;
    create_blob_with_object(&state, &source, b"do-not-move", 1).await;

    let body = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    let task_id = body["data"]["id"].as_i64().expect("task id should exist");

    let mut target_update: storage_policy::ActiveModel = target.clone().into();
    target_update.base_path = Set(format!("{}-changed", target.base_path));
    target_update.updated_at = Set(Utc::now() + chrono::Duration::seconds(1));
    target_update
        .update(state.writer_db())
        .await
        .expect("target policy should update");

    let stats = task_service::drain(&state)
        .await
        .expect("changed migration task should drain");
    assert_eq!(stats.failed, 1);

    let task = background_task_repo::find_by_id(state.writer_db(), task_id)
        .await
        .expect("task should exist");
    assert_eq!(task.status, BackgroundTaskStatus::Failed);
    assert!(
        task.last_error
            .as_deref()
            .expect("failed task should store last_error")
            .contains("storage policy changed after migration task was created")
    );

    let checkpoint = storage_migration_checkpoint_repo::get_by_task_id(state.writer_db(), task_id)
        .await
        .expect("checkpoint should exist");
    assert_eq!(checkpoint.stage, "prepare_policies");
    assert_eq!(checkpoint.last_processed_blob_id, 0);
    assert_eq!(checkpoint.scanned_blobs, 0);
    assert_eq!(checkpoint.migrated_blobs, 0);
    assert_eq!(checkpoint.merged_blobs, 0);
    assert_eq!(checkpoint.skipped_blobs, 0);
    assert_eq!(checkpoint.failed_blobs, 0);
}

#[actix_web::test]
async fn test_storage_migration_rejects_duplicate_active_pair() {
    let state = common::setup().await;
    let app = create_test_app!(state.clone());
    let (token, _) = register_and_login!(app);
    let source = create_local_policy(&state, "source-duplicate").await;
    let target = create_local_policy(&state, "target-duplicate").await;

    let first = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    assert_eq!(first["code"], 0);
    let second = create_migration_task_via_api(&app, &token, source.id, target.id, false).await;
    assert_ne!(second["code"], 0);
    assert!(
        second["msg"]
            .as_str()
            .expect("error message should exist")
            .contains("active storage policy migration")
    );
}
