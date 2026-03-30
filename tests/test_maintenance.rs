mod common;

use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
struct DirModeGuard {
    path: std::path::PathBuf,
    mode: u32,
}

#[cfg(unix)]
impl DirModeGuard {
    fn set_read_only(path: impl Into<std::path::PathBuf>) -> Self {
        let path = path.into();
        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode();
        let mut perms = metadata.permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&path, perms).unwrap();
        Self { path, mode }
    }
}

#[cfg(unix)]
impl Drop for DirModeGuard {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::metadata(&self.path) {
            let mut perms = metadata.permissions();
            perms.set_mode(self.mode);
            let _ = std::fs::set_permissions(&self.path, perms);
        }
    }
}

async fn default_policy(
    state: &aster_drive::runtime::AppState,
) -> aster_drive::entities::storage_policy::Model {
    aster_drive::db::repository::policy_repo::find_default(&state.db)
        .await
        .unwrap()
        .expect("default policy should exist in test setup")
}

#[allow(clippy::too_many_arguments)]
async fn create_upload_session(
    state: &aster_drive::runtime::AppState,
    user_id: i64,
    upload_id: &str,
    status: aster_drive::types::UploadSessionStatus,
    expires_at: chrono::DateTime<chrono::Utc>,
    s3_temp_key: Option<&str>,
    s3_multipart_id: Option<&str>,
    file_id: Option<i64>,
) {
    use aster_drive::db::repository::upload_session_repo;

    let policy = default_policy(state).await;
    let now = chrono::Utc::now();
    upload_session_repo::create(
        &state.db,
        aster_drive::entities::upload_session::ActiveModel {
            id: Set(upload_id.to_string()),
            user_id: Set(user_id),
            filename: Set("manual-upload.bin".to_string()),
            total_size: Set(10),
            chunk_size: Set(5),
            total_chunks: Set(2),
            received_count: Set(2),
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

async fn store_test_file(
    state: &aster_drive::runtime::AppState,
    user_id: i64,
    filename: &str,
    bytes: &[u8],
) -> aster_drive::entities::file::Model {
    let temp_path = aster_drive::utils::paths::temp_file_path(
        &state.config.server.temp_dir,
        &uuid::Uuid::new_v4().to_string(),
    );
    tokio::fs::create_dir_all(&state.config.server.temp_dir)
        .await
        .unwrap();
    tokio::fs::write(&temp_path, bytes).await.unwrap();

    aster_drive::services::file_service::store_from_temp(
        state,
        user_id,
        None,
        filename,
        &temp_path,
        bytes.len() as i64,
        None,
        false,
    )
    .await
    .unwrap()
}

fn thumb_path(blob_hash: &str) -> String {
    format!(
        "_thumb/{}/{}/{}.webp",
        &blob_hash[..2],
        &blob_hash[2..4],
        blob_hash
    )
}

async fn create_blob(
    state: &aster_drive::runtime::AppState,
    hash: &str,
    storage_path: &str,
    bytes: &[u8],
    ref_count: i32,
) -> aster_drive::entities::file_blob::Model {
    use aster_drive::db::repository::file_repo;

    let policy = default_policy(state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    driver.put(storage_path, bytes).await.unwrap();

    let now = Utc::now();
    file_repo::create_blob(
        &state.db,
        aster_drive::entities::file_blob::ActiveModel {
            hash: Set(hash.to_string()),
            size: Set(bytes.len() as i64),
            policy_id: Set(policy.id),
            storage_path: Set(storage_path.to_string()),
            ref_count: Set(ref_count),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

#[actix_web::test]
async fn test_cleanup_expired_completed_upload_sessions_removes_broken_temp_object() {
    use aster_drive::db::repository::upload_session_repo;
    use aster_drive::services::{auth_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintuser1", "maint1@test.com", "password123")
        .await
        .unwrap();
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let temp_key = "tmp/broken-completed.bin";
    driver.put(temp_key, b"stale upload").await.unwrap();

    create_upload_session(
        &state,
        user.id,
        "broken-completed",
        aster_drive::types::UploadSessionStatus::Completed,
        Utc::now() - Duration::hours(1),
        Some(temp_key),
        None,
        None,
    )
    .await;

    let stats = maintenance_service::cleanup_expired_completed_upload_sessions(&state)
        .await
        .unwrap();

    assert_eq!(stats.completed_sessions_deleted, 1);
    assert_eq!(stats.broken_completed_sessions_deleted, 1);
    assert!(
        upload_session_repo::find_by_id(&state.db, "broken-completed")
            .await
            .is_err()
    );
    assert!(!driver.exists(temp_key).await.unwrap());
}

#[actix_web::test]
async fn test_cleanup_expired_completed_upload_sessions_removes_broken_completed_multipart_object()
{
    use aster_drive::db::repository::upload_session_repo;
    use aster_drive::services::{auth_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintuser1b", "maint1b@test.com", "password123")
        .await
        .unwrap();
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();
    let temp_key = "tmp/broken-completed-multipart.bin";
    driver
        .put(temp_key, b"stale completed multipart")
        .await
        .unwrap();

    create_upload_session(
        &state,
        user.id,
        "broken-completed-multipart",
        aster_drive::types::UploadSessionStatus::Completed,
        Utc::now() - Duration::hours(1),
        Some(temp_key),
        Some("already-completed-upload"),
        None,
    )
    .await;

    let stats = maintenance_service::cleanup_expired_completed_upload_sessions(&state)
        .await
        .unwrap();

    assert_eq!(stats.completed_sessions_deleted, 1);
    assert_eq!(stats.broken_completed_sessions_deleted, 1);
    assert!(
        upload_session_repo::find_by_id(&state.db, "broken-completed-multipart")
            .await
            .is_err()
    );
    assert!(!driver.exists(temp_key).await.unwrap());
}

#[actix_web::test]
async fn test_cleanup_expired_completed_upload_sessions_keeps_live_blob() {
    use aster_drive::db::repository::{file_repo, upload_session_repo};
    use aster_drive::services::{auth_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintuser2", "maint2@test.com", "password123")
        .await
        .unwrap();
    let file = store_test_file(&state, user.id, "kept.txt", b"kept blob").await;
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();

    create_upload_session(
        &state,
        user.id,
        "completed-with-file",
        aster_drive::types::UploadSessionStatus::Completed,
        Utc::now() - Duration::hours(1),
        Some(&blob.storage_path),
        None,
        Some(file.id),
    )
    .await;

    let stats = maintenance_service::cleanup_expired_completed_upload_sessions(&state)
        .await
        .unwrap();

    assert_eq!(stats.completed_sessions_deleted, 1);
    assert_eq!(stats.broken_completed_sessions_deleted, 0);
    assert!(
        upload_session_repo::find_by_id(&state.db, "completed-with-file")
            .await
            .is_err()
    );
    assert!(file_repo::find_by_id(&state.db, file.id).await.is_ok());
    assert!(file_repo::find_blob_by_id(&state.db, blob.id).await.is_ok());
    assert!(driver.exists(&blob.storage_path).await.unwrap());
}

#[actix_web::test]
async fn test_cleanup_expired_completed_upload_sessions_processes_all_batches() {
    use aster_drive::entities::upload_session::Entity as UploadSession;
    use aster_drive::services::{auth_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintbatch", "maintbatch@test.com", "password123")
        .await
        .unwrap();

    for i in 0..1001 {
        let upload_id = format!("batch-session-{i:04}");
        let file_id = if i % 2 == 0 { None } else { Some(i as i64 + 1) };
        create_upload_session(
            &state,
            user.id,
            &upload_id,
            aster_drive::types::UploadSessionStatus::Completed,
            Utc::now() - Duration::hours(1),
            None,
            None,
            file_id,
        )
        .await;
    }

    let stats = maintenance_service::cleanup_expired_completed_upload_sessions(&state)
        .await
        .unwrap();

    assert_eq!(stats.completed_sessions_deleted, 1001);
    assert_eq!(stats.broken_completed_sessions_deleted, 501);
    assert_eq!(UploadSession::find().count(&state.db).await.unwrap(), 0);
}

#[actix_web::test]
async fn test_reconcile_blob_state_deletes_orphans_and_fixes_ref_counts() {
    use aster_drive::db::repository::{file_repo, version_repo};
    use aster_drive::services::{auth_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintuser3", "maint3@test.com", "password123")
        .await
        .unwrap();
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();

    let live_file = store_test_file(&state, user.id, "live.txt", b"live blob").await;
    let live_blob = file_repo::find_blob_by_id(&state.db, live_file.blob_id)
        .await
        .unwrap();
    let mut live_blob_active: aster_drive::entities::file_blob::ActiveModel =
        live_blob.clone().into();
    live_blob_active.ref_count = Set(7);
    live_blob_active.updated_at = Set(Utc::now());
    live_blob_active.update(&state.db).await.unwrap();

    let version_hash = "b".repeat(64);
    let version_blob = create_blob(
        &state,
        &version_hash,
        "versions/version-only.bin",
        b"version blob",
        9,
    )
    .await;
    version_repo::create(
        &state.db,
        aster_drive::entities::file_version::ActiveModel {
            file_id: Set(live_file.id),
            blob_id: Set(version_blob.id),
            version: Set(1),
            size: Set(version_blob.size),
            created_at: Set(Utc::now()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let orphan_hash = "c".repeat(64);
    let orphan_path = "orphans/orphan.bin";
    let orphan_blob = create_blob(&state, &orphan_hash, orphan_path, b"orphan blob", 1).await;
    let orphan_thumb = thumb_path(&orphan_hash);
    driver.put(&orphan_thumb, b"thumb").await.unwrap();

    let stats = maintenance_service::reconcile_blob_state(&state)
        .await
        .unwrap();

    assert_eq!(stats.ref_count_fixed, 3);
    assert_eq!(stats.orphan_blobs_deleted, 1);

    let live_blob_after = file_repo::find_blob_by_id(&state.db, live_blob.id)
        .await
        .unwrap();
    assert_eq!(live_blob_after.ref_count, 1);

    let version_blob_after = file_repo::find_blob_by_id(&state.db, version_blob.id)
        .await
        .unwrap();
    assert_eq!(version_blob_after.ref_count, 1);

    assert!(
        file_repo::find_blob_by_id(&state.db, orphan_blob.id)
            .await
            .is_err()
    );
    assert!(!driver.exists(orphan_path).await.unwrap());
    assert!(!driver.exists(&orphan_thumb).await.unwrap());
}

#[actix_web::test]
async fn test_reconcile_blob_state_processes_all_batches_without_skipping() {
    use aster_drive::entities::file_blob::Entity as FileBlob;
    use aster_drive::services::maintenance_service;

    let state = common::setup().await;
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();

    for i in 0..1001u64 {
        let hash = format!("{i:064x}");
        let storage_path = format!("paging/blob-{i:04}.bin");
        create_blob(&state, &hash, &storage_path, b"x", 1).await;
    }

    let stats = maintenance_service::reconcile_blob_state(&state)
        .await
        .unwrap();

    assert_eq!(stats.ref_count_fixed, 1001);
    assert_eq!(stats.orphan_blobs_deleted, 1001);
    assert_eq!(FileBlob::find().count(&state.db).await.unwrap(), 0);
    assert!(!driver.exists("paging/blob-0000.bin").await.unwrap());
    assert!(!driver.exists("paging/blob-1000.bin").await.unwrap());
}

#[cfg(unix)]
#[actix_web::test]
async fn test_purge_keeps_blob_row_when_storage_delete_fails_then_maintenance_retries() {
    use aster_drive::db::repository::file_repo;
    use aster_drive::services::{auth_service, file_service, maintenance_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintuser4", "maint4@test.com", "password123")
        .await
        .unwrap();
    let policy = default_policy(&state).await;
    let driver = state.driver_registry.get_driver(&policy).unwrap();

    let file = store_test_file(&state, user.id, "retry.txt", b"retry blob").await;
    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id)
        .await
        .unwrap();

    let object_parent = std::path::Path::new(&policy.base_path)
        .join(&blob.storage_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let _guard = DirModeGuard::set_read_only(object_parent);

    file_service::purge(&state, file.id, user.id).await.unwrap();

    assert!(file_repo::find_by_id(&state.db, file.id).await.is_err());
    let blob_after_purge = file_repo::find_blob_by_id(&state.db, blob.id)
        .await
        .unwrap();
    assert_eq!(blob_after_purge.ref_count, 0);
    assert!(driver.exists(&blob.storage_path).await.unwrap());

    drop(_guard);

    let stats = maintenance_service::reconcile_blob_state(&state)
        .await
        .unwrap();

    assert_eq!(stats.orphan_blobs_deleted, 1);
    assert!(
        file_repo::find_blob_by_id(&state.db, blob.id)
            .await
            .is_err()
    );
    assert!(!driver.exists(&blob.storage_path).await.unwrap());
}

#[actix_web::test]
async fn test_purge_releases_all_versioned_storage_used() {
    use aster_drive::db::repository::user_repo;
    use aster_drive::services::{auth_service, file_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintquota1", "maintquota1@test.com", "password123")
        .await
        .unwrap();

    let initial_bytes = b"first-version";
    let updated_bytes = b"second-version-kept";
    let file = store_test_file(&state, user.id, "quota-purge.txt", initial_bytes).await;

    file_service::update_content(
        &state,
        file.id,
        user.id,
        actix_web::web::Bytes::from_static(updated_bytes),
        None,
    )
    .await
    .unwrap();

    let before_purge = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(
        before_purge.storage_used,
        initial_bytes.len() as i64 + updated_bytes.len() as i64
    );

    file_service::purge(&state, file.id, user.id).await.unwrap();

    let after_purge = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(after_purge.storage_used, 0);
}

#[actix_web::test]
async fn test_batch_purge_releases_all_versioned_storage_used() {
    use aster_drive::db::repository::{file_repo, user_repo};
    use aster_drive::services::{auth_service, file_service};

    let state = common::setup().await;
    let user = auth_service::register(&state, "maintquota2", "maintquota2@test.com", "password123")
        .await
        .unwrap();

    let file_a_v1 = b"alpha-version-one";
    let file_a_v2 = b"alpha-version-two-long";
    let file_b_v1 = b"beta-version-one";
    let file_b_v2 = b"beta-version-two-even-longer";

    let file_a = store_test_file(&state, user.id, "quota-a.txt", file_a_v1).await;
    let file_b = store_test_file(&state, user.id, "quota-b.txt", file_b_v1).await;

    file_service::update_content(
        &state,
        file_a.id,
        user.id,
        actix_web::web::Bytes::from_static(file_a_v2),
        None,
    )
    .await
    .unwrap();
    file_service::update_content(
        &state,
        file_b.id,
        user.id,
        actix_web::web::Bytes::from_static(file_b_v2),
        None,
    )
    .await
    .unwrap();

    let before_purge = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(
        before_purge.storage_used,
        (file_a_v1.len() + file_a_v2.len() + file_b_v1.len() + file_b_v2.len()) as i64
    );

    let files = file_repo::find_by_ids(&state.db, &[file_a.id, file_b.id])
        .await
        .unwrap();
    let purged = file_service::batch_purge(&state, files, user.id)
        .await
        .unwrap();
    assert_eq!(purged, 2);

    let after_purge = user_repo::find_by_id(&state.db, user.id).await.unwrap();
    assert_eq!(after_purge.storage_used, 0);
}
