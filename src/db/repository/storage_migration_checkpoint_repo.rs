//! `storage_migration_checkpoint` 仓储模块。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, RelationTrait, Set,
};

use crate::entities::storage_migration_checkpoint::{self, Entity as StorageMigrationCheckpoint};
use crate::errors::{AsterError, Result};

#[derive(Debug, Clone)]
pub struct CreateCheckpointInput<'a> {
    pub task_id: i64,
    pub source_policy_id: i64,
    pub target_policy_id: i64,
    pub plan_hash: &'a str,
    pub stage: &'a str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CheckpointDelta {
    pub scanned_blobs: i64,
    pub migrated_blobs: i64,
    pub merged_blobs: i64,
    pub skipped_blobs: i64,
    pub failed_blobs: i64,
    pub migrated_bytes: i64,
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    input: CreateCheckpointInput<'_>,
) -> Result<storage_migration_checkpoint::Model> {
    let now = Utc::now();
    storage_migration_checkpoint::ActiveModel {
        task_id: Set(input.task_id),
        source_policy_id: Set(input.source_policy_id),
        target_policy_id: Set(input.target_policy_id),
        plan_hash: Set(input.plan_hash.to_string()),
        stage: Set(input.stage.to_string()),
        last_processed_blob_id: Set(0),
        scanned_blobs: Set(0),
        migrated_blobs: Set(0),
        merged_blobs: Set(0),
        skipped_blobs: Set(0),
        failed_blobs: Set(0),
        migrated_bytes: Set(0),
        last_error: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .map_err(AsterError::from)
}

pub async fn find_by_task_id<C: ConnectionTrait>(
    db: &C,
    task_id: i64,
) -> Result<Option<storage_migration_checkpoint::Model>> {
    StorageMigrationCheckpoint::find_by_id(task_id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn get_by_task_id<C: ConnectionTrait>(
    db: &C,
    task_id: i64,
) -> Result<storage_migration_checkpoint::Model> {
    find_by_task_id(db, task_id).await?.ok_or_else(|| {
        AsterError::record_not_found(format!("storage migration checkpoint #{task_id}"))
    })
}

pub async fn advance<C: ConnectionTrait>(
    db: &C,
    task_id: i64,
    stage: &str,
    last_processed_blob_id: i64,
    delta: CheckpointDelta,
    last_error: Option<&str>,
) -> Result<storage_migration_checkpoint::Model> {
    let checkpoint = get_by_task_id(db, task_id).await?;
    let updated = Utc::now();
    let active = storage_migration_checkpoint::ActiveModel {
        task_id: Set(checkpoint.task_id),
        source_policy_id: Set(checkpoint.source_policy_id),
        target_policy_id: Set(checkpoint.target_policy_id),
        plan_hash: Set(checkpoint.plan_hash),
        stage: Set(stage.to_string()),
        last_processed_blob_id: Set(last_processed_blob_id),
        scanned_blobs: Set(checkpoint.scanned_blobs + delta.scanned_blobs),
        migrated_blobs: Set(checkpoint.migrated_blobs + delta.migrated_blobs),
        merged_blobs: Set(checkpoint.merged_blobs + delta.merged_blobs),
        skipped_blobs: Set(checkpoint.skipped_blobs + delta.skipped_blobs),
        failed_blobs: Set(checkpoint.failed_blobs + delta.failed_blobs),
        migrated_bytes: Set(checkpoint.migrated_bytes + delta.migrated_bytes),
        last_error: Set(last_error.map(str::to_string)),
        created_at: Set(checkpoint.created_at),
        updated_at: Set(updated),
    };
    active.update(db).await.map_err(AsterError::from)
}

pub async fn set_stage<C: ConnectionTrait>(
    db: &C,
    task_id: i64,
    stage: &str,
    last_error: Option<&str>,
) -> Result<storage_migration_checkpoint::Model> {
    let checkpoint = get_by_task_id(db, task_id).await?;
    let active = storage_migration_checkpoint::ActiveModel {
        task_id: Set(checkpoint.task_id),
        source_policy_id: Set(checkpoint.source_policy_id),
        target_policy_id: Set(checkpoint.target_policy_id),
        plan_hash: Set(checkpoint.plan_hash),
        stage: Set(stage.to_string()),
        last_processed_blob_id: Set(checkpoint.last_processed_blob_id),
        scanned_blobs: Set(checkpoint.scanned_blobs),
        migrated_blobs: Set(checkpoint.migrated_blobs),
        merged_blobs: Set(checkpoint.merged_blobs),
        skipped_blobs: Set(checkpoint.skipped_blobs),
        failed_blobs: Set(checkpoint.failed_blobs),
        migrated_bytes: Set(checkpoint.migrated_bytes),
        last_error: Set(last_error.map(str::to_string)),
        created_at: Set(checkpoint.created_at),
        updated_at: Set(Utc::now()),
    };
    active.update(db).await.map_err(AsterError::from)
}

pub async fn has_active_for_pair<C: ConnectionTrait>(
    db: &C,
    source_policy_id: i64,
    target_policy_id: i64,
) -> Result<bool> {
    use crate::entities::background_task;
    use crate::types::BackgroundTaskStatus;
    use sea_orm::QuerySelect;

    let existing = StorageMigrationCheckpoint::find()
        .join(
            sea_orm::JoinType::InnerJoin,
            storage_migration_checkpoint::Relation::BackgroundTask.def(),
        )
        .filter(storage_migration_checkpoint::Column::SourcePolicyId.eq(source_policy_id))
        .filter(storage_migration_checkpoint::Column::TargetPolicyId.eq(target_policy_id))
        .filter(background_task::Column::Status.is_in([
            BackgroundTaskStatus::Pending,
            BackgroundTaskStatus::Processing,
            BackgroundTaskStatus::Retry,
        ]))
        .select_only()
        .column(storage_migration_checkpoint::Column::TaskId)
        .into_tuple::<i64>()
        .one(db)
        .await
        .map_err(AsterError::from)?;

    Ok(existing.is_some())
}
