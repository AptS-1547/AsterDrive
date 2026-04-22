//! 文件服务子模块：`deletion`。

use futures::{StreamExt, stream};

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::{
    media_processing_service, storage_change_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::utils::numbers::usize_to_u32;

use super::get_info_in_scope;

const BLOB_CLEANUP_CONCURRENCY: usize = 8;

pub(crate) async fn delete_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    tracing::debug!(scope = ?scope, file_id = id, "soft deleting file");
    let file = get_info_in_scope(state, scope, id).await?;
    if file.is_locked {
        return Err(AsterError::resource_locked("file is locked"));
    }
    file_repo::soft_delete(&state.db, id).await?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FileDeleted,
            scope,
            vec![file.id],
            vec![],
            vec![file.folder_id],
        ),
    );
    tracing::debug!(
        scope = ?scope,
        file_id = file.id,
        folder_id = file.folder_id,
        "soft deleted file"
    );
    Ok(())
}

/// 删除文件（软删除 → 回收站）
pub async fn delete(state: &PrimaryAppState, id: i64, user_id: i64) -> Result<()> {
    delete_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

pub(crate) async fn ensure_blob_cleanup_if_unreferenced(
    state: &PrimaryAppState,
    blob_id: i64,
) -> bool {
    let current_blob = match file_repo::find_blob_by_id(&state.db, blob_id).await {
        Ok(current_blob) => current_blob,
        Err(e) if e.code() == "E006" => return true,
        Err(e) => {
            tracing::warn!(
                blob_id,
                "failed to reload blob before deciding whether cleanup is needed: {e}"
            );
            return false;
        }
    };

    if current_blob.ref_count != 0 {
        return true;
    }

    match file_repo::claim_blob_cleanup(&state.db, current_blob.id).await {
        Ok(true) => cleanup_claimed_blob(state, &current_blob).await,
        Ok(false) => true,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "failed to claim blob cleanup: {e}"
            );
            false
        }
    }
}

pub(crate) async fn cleanup_unreferenced_blob(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
) -> bool {
    let current_blob = match file_repo::find_blob_by_id(&state.db, blob.id).await {
        Ok(current_blob) => current_blob,
        Err(e) if e.code() == "E006" => return true,
        Err(e) => {
            tracing::warn!(
                blob_id = blob.id,
                "failed to reload blob before cleanup: {e}"
            );
            return false;
        }
    };

    if current_blob.ref_count != 0 {
        tracing::warn!(
            blob_id = current_blob.id,
            ref_count = current_blob.ref_count,
            "skipping blob cleanup because blob is referenced again"
        );
        return false;
    }

    match file_repo::claim_blob_cleanup(&state.db, current_blob.id).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "skipping blob cleanup because another worker already claimed it or it was revived"
            );
            return false;
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "failed to claim blob cleanup: {e}"
            );
            return false;
        }
    }

    cleanup_claimed_blob(state, &current_blob).await
}

async fn cleanup_claimed_blob(state: &PrimaryAppState, current_blob: &file_blob::Model) -> bool {
    async fn restore_cleanup_claim(state: &PrimaryAppState, blob_id: i64, reason: &str) {
        match file_repo::restore_blob_cleanup_claim(&state.db, blob_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    blob_id,
                    "blob cleanup claim was already released while handling {reason}"
                );
            }
            Err(e) => {
                tracing::warn!(
                    blob_id,
                    "failed to restore blob cleanup claim after {reason}: {e}"
                );
            }
        }
    }

    if let Err(e) = media_processing_service::delete_thumbnail(state, current_blob).await {
        tracing::warn!(
            blob_id = current_blob.id,
            "failed to delete thumbnail during blob cleanup: {e}"
        );
    }

    let Some(policy) = state.policy_snapshot.get_policy(current_blob.policy_id) else {
        tracing::warn!(
            blob_id = current_blob.id,
            policy_id = current_blob.policy_id,
            "failed to load storage policy during blob cleanup: policy missing from snapshot"
        );
        restore_cleanup_claim(state, current_blob.id, "policy lookup failure").await;
        return false;
    };

    let driver = match state.driver_registry.get_driver(&policy) {
        Ok(driver) => driver,
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                policy_id = current_blob.policy_id,
                "failed to resolve storage driver during blob cleanup: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "driver resolution failure").await;
            return false;
        }
    };

    let object_deleted = match driver.delete(&current_blob.storage_path).await {
        Ok(()) => true,
        Err(e) => match driver.exists(&current_blob.storage_path).await {
            Ok(false) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "blob delete returned error but object is already absent: {e}"
                );
                true
            }
            Ok(true) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object, keeping blob row for retry: {e}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete error").await;
                false
            }
            Err(exists_err) => {
                tracing::warn!(
                    blob_id = current_blob.id,
                    path = %current_blob.storage_path,
                    "failed to delete blob object and verify existence, keeping blob row for retry: delete_error={e}, exists_error={exists_err}"
                );
                restore_cleanup_claim(state, current_blob.id, "delete verification error").await;
                false
            }
        },
    };

    if !object_deleted {
        return false;
    }

    match file_repo::delete_blob_if_cleanup_claimed(&state.db, current_blob.id).await {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but cleanup claim was lost before deleting blob row"
            );
            restore_cleanup_claim(
                state,
                current_blob.id,
                "lost cleanup claim before row delete",
            )
            .await;
            false
        }
        Err(e) => {
            tracing::warn!(
                blob_id = current_blob.id,
                "blob object is gone but failed to delete blob row: {e}"
            );
            restore_cleanup_claim(state, current_blob.id, "row delete failure").await;
            false
        }
    }
}

pub(crate) async fn purge_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    id: i64,
) -> Result<()> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let file = file_repo::find_by_id(&state.db, id).await?;
    workspace_storage_service::ensure_file_scope(&file, scope)?;

    batch_purge_in_scope(state, scope, vec![file]).await?;
    Ok(())
}

/// 永久删除文件，处理 blob ref_count、物理文件、缩略图和配额。
pub async fn purge(state: &PrimaryAppState, id: i64, user_id: i64) -> Result<()> {
    purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, id).await
}

/// 批量永久删除文件：一次事务处理所有 DB 操作，事务后并行清理物理文件
///
/// 比逐个调 `purge()` 快得多——N 个文件只需 ~10 次 DB 查询而非 ~12N 次。
pub(crate) async fn batch_purge_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    files: Vec<file::Model>,
) -> Result<u32> {
    if files.is_empty() {
        return Ok(0);
    }

    let input_count = files.len();
    tracing::debug!(
        scope = ?scope,
        file_count = input_count,
        "purging files permanently"
    );

    for file in &files {
        workspace_storage_service::ensure_file_scope(file, scope)?;
    }

    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let blob_ids: Vec<i64> = files.iter().map(|f| f.blob_id).collect();
    let count = usize_to_u32(files.len(), "purged file count")?;

    // ── 单次事务：版本 → 属性 → 文件 → blob → 配额 ──
    let txn = crate::db::transaction::begin(&state.db).await?;

    // 1. 批量删除版本记录，收集版本 blob IDs
    let version_blob_ids =
        crate::db::repository::version_repo::delete_all_by_file_ids(&txn, &file_ids).await?;

    // 2. 批量删除文件属性
    crate::db::repository::property_repo::delete_all_for_entities(
        &txn,
        crate::types::EntityType::File,
        &file_ids,
    )
    .await?;

    // 3. 批量删除文件记录（先于 blob，解除 FK）
    file_repo::delete_many(&txn, &file_ids).await?;

    // 4. 处理 blob 引用计数
    //    合并主 blob 和版本 blob，按 blob_id 统计需要减少的引用数
    let mut blob_decrements: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &bid in &blob_ids {
        *blob_decrements.entry(bid).or_default() += 1;
    }
    for &vbid in &version_blob_ids {
        *blob_decrements.entry(vbid).or_default() += 1;
    }

    let blob_ids: Vec<i64> = blob_decrements.keys().copied().collect();
    let blobs_by_id = file_repo::find_blobs_by_ids(&txn, &blob_ids).await?;
    let mut total_freed_bytes = 0i64;

    for (&blob_id, &decrement) in &blob_decrements {
        if let Some(blob) = blobs_by_id.get(&blob_id) {
            let freed_bytes = blob.size.checked_mul(decrement).ok_or_else(|| {
                AsterError::internal_error(format!(
                    "freed byte count overflow for blob {blob_id} during batch purge"
                ))
            })?;
            total_freed_bytes = total_freed_bytes.checked_add(freed_bytes).ok_or_else(|| {
                AsterError::internal_error("total freed byte count overflow during batch purge")
            })?;
            let decrement_i32 = i32::try_from(decrement).map_err(|_| {
                AsterError::internal_error(format!(
                    "blob decrement overflow for blob {blob_id} during batch purge"
                ))
            })?;
            file_repo::decrement_blob_ref_count_by(&txn, blob_id, decrement_i32).await?;
        }
    }

    // 5. 配额一次性更新
    workspace_storage_service::update_storage_used(&txn, scope, -total_freed_bytes).await?;

    crate::db::transaction::commit(txn).await?;

    // ── 事务后：按提交后的真实 ref_count 重检，避免并发 purge 依据旧快照漏掉归零清理 ──
    stream::iter(blob_ids.iter().copied())
        .for_each_concurrent(BLOB_CLEANUP_CONCURRENCY, |blob_id| async move {
            if !ensure_blob_cleanup_if_unreferenced(state, blob_id).await {
                tracing::warn!(
                    blob_id,
                    "batch purge left blob row for retry because object cleanup was incomplete"
                );
            }
        })
        .await;

    tracing::debug!(
        scope = ?scope,
        file_count = input_count,
        freed_bytes = total_freed_bytes,
        cleanup_blob_count = blob_ids.len(),
        "purged files permanently"
    );
    Ok(count)
}

pub async fn batch_purge(
    state: &PrimaryAppState,
    files: Vec<file::Model>,
    user_id: i64,
) -> Result<u32> {
    batch_purge_in_scope(state, WorkspaceStorageScope::Personal { user_id }, files).await
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use tokio::io::{AsyncRead, empty};

    use super::*;
    use crate::cache;
    use crate::config::{CacheConfig, Config, DatabaseConfig, RuntimeConfig};
    use crate::entities::{storage_policy, user};
    use crate::services::mail_service;
    use crate::storage::driver::BlobMetadata;
    use crate::storage::{DriverRegistry, PolicySnapshot, StorageDriver};
    use crate::types::{
        DriverType, StoredStoragePolicyAllowedTypes, StoredStoragePolicyOptions, UserRole,
        UserStatus,
    };

    #[derive(Clone, Default)]
    struct TrackingDeleteDriver {
        objects: Arc<Mutex<HashSet<String>>>,
        delete_calls: Arc<AtomicUsize>,
    }

    impl TrackingDeleteDriver {
        fn insert_object(&self, path: &str) {
            self.objects
                .lock()
                .expect("tracking delete driver lock should succeed")
                .insert(path.to_string());
        }

        fn delete_calls(&self) -> usize {
            self.delete_calls.load(Ordering::SeqCst)
        }

        fn contains(&self, path: &str) -> bool {
            self.objects
                .lock()
                .expect("tracking delete driver lock should succeed")
                .contains(path)
        }
    }

    #[async_trait]
    impl StorageDriver for TrackingDeleteDriver {
        async fn put(&self, path: &str, _data: &[u8]) -> crate::errors::Result<String> {
            self.insert_object(path);
            Ok(path.to_string())
        }

        async fn get(&self, _path: &str) -> crate::errors::Result<Vec<u8>> {
            Ok(Vec::new())
        }

        async fn get_stream(
            &self,
            _path: &str,
        ) -> crate::errors::Result<Box<dyn AsyncRead + Unpin + Send>> {
            Ok(Box::new(empty()))
        }

        async fn delete(&self, path: &str) -> crate::errors::Result<()> {
            self.delete_calls.fetch_add(1, Ordering::SeqCst);
            self.objects
                .lock()
                .expect("tracking delete driver lock should succeed")
                .remove(path);
            Ok(())
        }

        async fn exists(&self, path: &str) -> crate::errors::Result<bool> {
            Ok(self.contains(path))
        }

        async fn metadata(&self, path: &str) -> crate::errors::Result<BlobMetadata> {
            Ok(BlobMetadata {
                size: if self.contains(path) { 1 } else { 0 },
                content_type: Some("application/octet-stream".to_string()),
            })
        }
    }

    async fn build_deletion_test_state() -> (
        PrimaryAppState,
        user::Model,
        storage_policy::Model,
        TrackingDeleteDriver,
    ) {
        let temp_root = std::env::temp_dir().join(format!(
            "asterdrive-deletion-service-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).expect("deletion test temp root should exist");

        let db = crate::db::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            pool_size: 1,
            retry_count: 0,
        })
        .await
        .expect("deletion test DB should connect");
        Migrator::up(&db, None)
            .await
            .expect("deletion test migrations should succeed");

        let now = Utc::now();
        let policy = storage_policy::ActiveModel {
            name: Set("Deletion Test Policy".to_string()),
            driver_type: Set(DriverType::Local),
            endpoint: Set(String::new()),
            bucket: Set(String::new()),
            access_key: Set(String::new()),
            secret_key: Set(String::new()),
            base_path: Set(temp_root.join("uploads").to_string_lossy().into_owned()),
            max_file_size: Set(0),
            allowed_types: Set(StoredStoragePolicyAllowedTypes::empty()),
            options: Set(StoredStoragePolicyOptions::empty()),
            is_default: Set(true),
            chunk_size: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("deletion test policy should insert");

        let user = user::ActiveModel {
            username: Set(format!("deletion-user-{}", uuid::Uuid::new_v4())),
            email: Set(format!("deletion-{}@example.com", uuid::Uuid::new_v4())),
            password_hash: Set("not-used".to_string()),
            role: Set(UserRole::User),
            status: Set(UserStatus::Active),
            session_version: Set(0),
            email_verified_at: Set(Some(now)),
            pending_email: Set(None),
            storage_used: Set(0),
            storage_quota: Set(0),
            policy_group_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            config: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("deletion test user should insert");

        let runtime_config = Arc::new(RuntimeConfig::new());
        let cache = cache::create_cache(&CacheConfig {
            enabled: false,
            ..Default::default()
        })
        .await;
        let mut config = Config::default();
        config.server.temp_dir = temp_root.join(".tmp").to_string_lossy().into_owned();
        config.server.upload_temp_dir = temp_root.join(".uploads").to_string_lossy().into_owned();

        let driver = TrackingDeleteDriver::default();
        let driver_registry = Arc::new(DriverRegistry::new());
        driver_registry.insert_for_test(policy.id, Arc::new(driver.clone()));
        let policy_snapshot = Arc::new(PolicySnapshot::new());
        policy_snapshot
            .reload(&db)
            .await
            .expect("policy snapshot should reload");

        let (storage_change_tx, _) = tokio::sync::broadcast::channel(
            crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
        );
        let share_download_rollback =
            crate::services::share_service::spawn_detached_share_download_rollback_queue(
                db.clone(),
                crate::config::operations::share_download_rollback_queue_capacity(&runtime_config),
            );
        let state = PrimaryAppState {
            db,
            driver_registry,
            runtime_config: runtime_config.clone(),
            policy_snapshot,
            config: Arc::new(config),
            cache,
            mail_sender: mail_service::runtime_sender(runtime_config),
            storage_change_tx,
            share_download_rollback,
        };

        (state, user, policy, driver)
    }

    async fn create_blob(
        db: &sea_orm::DatabaseConnection,
        policy_id: i64,
        storage_path: &str,
        size: i64,
        ref_count: i32,
    ) -> file_blob::Model {
        let now = Utc::now();
        file_blob::ActiveModel {
            hash: Set(format!("blob-{}", uuid::Uuid::new_v4())),
            size: Set(size),
            policy_id: Set(policy_id),
            storage_path: Set(storage_path.to_string()),
            thumbnail_path: Set(None),
            thumbnail_version: Set(None),
            ref_count: Set(ref_count),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("test blob should insert")
    }

    async fn create_file(
        db: &sea_orm::DatabaseConnection,
        user_id: i64,
        blob_id: i64,
        size: i64,
        name: &str,
    ) -> file::Model {
        let now = Utc::now();
        file::ActiveModel {
            name: Set(name.to_string()),
            folder_id: Set(None),
            team_id: Set(None),
            blob_id: Set(blob_id),
            size: Set(size),
            user_id: Set(user_id),
            mime_type: Set("application/octet-stream".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            is_locked: Set(false),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("test file should insert")
    }

    async fn set_user_storage_used(
        db: &sea_orm::DatabaseConnection,
        user_model: &user::Model,
        storage_used: i64,
    ) {
        let mut active: user::ActiveModel = user_model.clone().into();
        active.storage_used = Set(storage_used);
        active.updated_at = Set(Utc::now());
        active
            .update(db)
            .await
            .expect("test user storage should update");
    }

    #[tokio::test]
    async fn ensure_blob_cleanup_if_unreferenced_deletes_zero_ref_blob() {
        let (state, _user, policy, driver) = build_deletion_test_state().await;
        let blob = create_blob(&state.db, policy.id, "files/orphan.bin", 7, 0).await;
        driver.insert_object(&blob.storage_path);

        let cleaned = ensure_blob_cleanup_if_unreferenced(&state, blob.id).await;

        assert!(cleaned, "zero-ref blob should be cleaned");
        assert_eq!(driver.delete_calls(), 1, "object delete should run once");
        assert!(
            !driver.contains(&blob.storage_path),
            "blob object should be removed from the mock driver"
        );
        assert!(
            file_blob::Entity::find_by_id(blob.id)
                .one(&state.db)
                .await
                .expect("blob lookup should succeed")
                .is_none(),
            "blob row should be deleted after cleanup"
        );
    }

    #[tokio::test]
    async fn ensure_blob_cleanup_if_unreferenced_skips_referenced_blob() {
        let (state, _user, policy, driver) = build_deletion_test_state().await;
        let blob = create_blob(&state.db, policy.id, "files/in-use.bin", 9, 2).await;
        driver.insert_object(&blob.storage_path);

        let cleaned = ensure_blob_cleanup_if_unreferenced(&state, blob.id).await;

        assert!(
            cleaned,
            "positive ref_count should be treated as no cleanup needed"
        );
        assert_eq!(
            driver.delete_calls(),
            0,
            "referenced blob must not be deleted"
        );
        assert!(
            file_blob::Entity::find_by_id(blob.id)
                .one(&state.db)
                .await
                .expect("blob lookup should succeed")
                .is_some(),
            "referenced blob row must remain"
        );
    }

    #[tokio::test]
    async fn batch_purge_in_scope_deletes_last_blob_reference() {
        let (state, user, policy, driver) = build_deletion_test_state().await;
        let blob = create_blob(&state.db, policy.id, "files/last-ref.bin", 11, 1).await;
        driver.insert_object(&blob.storage_path);
        let file = create_file(&state.db, user.id, blob.id, 11, "last-ref.bin").await;
        set_user_storage_used(&state.db, &user, 11).await;

        let purged = batch_purge_in_scope(
            &state,
            WorkspaceStorageScope::Personal { user_id: user.id },
            vec![file.clone()],
        )
        .await
        .expect("batch purge should succeed");

        assert_eq!(purged, 1);
        assert_eq!(
            driver.delete_calls(),
            1,
            "last blob reference should delete object"
        );
        assert!(
            file::Entity::find_by_id(file.id)
                .one(&state.db)
                .await
                .expect("file lookup should succeed")
                .is_none(),
            "file row should be deleted"
        );
        assert!(
            file_blob::Entity::find_by_id(blob.id)
                .one(&state.db)
                .await
                .expect("blob lookup should succeed")
                .is_none(),
            "blob row should be deleted when the last reference is purged"
        );
        let reloaded_user = user::Entity::find_by_id(user.id)
            .one(&state.db)
            .await
            .expect("user lookup should succeed")
            .expect("user should remain");
        assert_eq!(
            reloaded_user.storage_used, 0,
            "purge should reclaim user storage"
        );
    }

    #[tokio::test]
    async fn batch_purge_in_scope_keeps_blob_when_other_file_still_references_it() {
        let (state, user, policy, driver) = build_deletion_test_state().await;
        let blob = create_blob(&state.db, policy.id, "files/shared.bin", 13, 2).await;
        driver.insert_object(&blob.storage_path);
        let file_a = create_file(&state.db, user.id, blob.id, 13, "shared-a.bin").await;
        let _file_b = create_file(&state.db, user.id, blob.id, 13, "shared-b.bin").await;
        set_user_storage_used(&state.db, &user, 26).await;

        let purged = batch_purge_in_scope(
            &state,
            WorkspaceStorageScope::Personal { user_id: user.id },
            vec![file_a.clone()],
        )
        .await
        .expect("batch purge should succeed");

        assert_eq!(purged, 1);
        assert_eq!(
            driver.delete_calls(),
            0,
            "shared blob must not be deleted while another file still references it"
        );
        let reloaded_blob = file_blob::Entity::find_by_id(blob.id)
            .one(&state.db)
            .await
            .expect("blob lookup should succeed")
            .expect("shared blob should remain");
        assert_eq!(
            reloaded_blob.ref_count, 1,
            "shared blob ref_count should decrement to 1"
        );
        let reloaded_user = user::Entity::find_by_id(user.id)
            .one(&state.db)
            .await
            .expect("user lookup should succeed")
            .expect("user should remain");
        assert_eq!(
            reloaded_user.storage_used, 13,
            "only one file's bytes should be reclaimed"
        );
    }
}
