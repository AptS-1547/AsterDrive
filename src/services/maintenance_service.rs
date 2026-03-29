use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, QueryOrder, QuerySelect,
    Set, sea_query::Expr,
};

use crate::db::repository::{file_repo, policy_repo, upload_session_repo};
use crate::entities::{
    file::{self, Entity as File},
    file_blob::{self, Entity as FileBlob},
    file_version::{self, Entity as FileVersion},
    upload_session::{self, Entity as UploadSession},
};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::UploadSessionStatus;

const COMPLETED_SESSION_BATCH_SIZE: u64 = 1_000;
const BLOB_RECONCILE_BATCH_SIZE: u64 = 1_000;
const MULTIPART_ABORT_MAX_ATTEMPTS: u32 = 3;
const MULTIPART_ABORT_INITIAL_BACKOFF_MS: u64 = 200;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UploadSessionMaintenanceStats {
    pub completed_sessions_deleted: u64,
    pub broken_completed_sessions_deleted: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BlobMaintenanceStats {
    pub ref_count_fixed: u64,
    pub orphan_blobs_deleted: u64,
}

pub async fn cleanup_expired_completed_upload_sessions(
    state: &AppState,
) -> Result<UploadSessionMaintenanceStats> {
    let now = Utc::now();
    let mut last_id: Option<String> = None;
    let mut stats = UploadSessionMaintenanceStats::default();

    loop {
        let mut query = UploadSession::find()
            .filter(upload_session::Column::ExpiresAt.lt(now))
            .filter(upload_session::Column::Status.eq(UploadSessionStatus::Completed))
            .order_by_asc(upload_session::Column::Id)
            .limit(COMPLETED_SESSION_BATCH_SIZE);
        if let Some(last_id_value) = last_id.as_ref() {
            query = query.filter(upload_session::Column::Id.gt(last_id_value.clone()));
        }

        let sessions = query.all(&state.db).await.map_err(AsterError::from)?;
        if sessions.is_empty() {
            break;
        }
        last_id = sessions.last().map(|session| session.id.clone());

        let broken_temp_keys: Vec<String> = sessions
            .iter()
            .filter(|session| session.file_id.is_none())
            .filter_map(|session| session.s3_temp_key.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let tracked_blob_paths = load_tracked_blob_paths(state, &broken_temp_keys).await?;

        for session in sessions {
            let broken_completed = session.file_id.is_none();

            if broken_completed {
                cleanup_broken_completed_session_object(state, &session, &tracked_blob_paths).await;
            }

            let temp_dir = crate::utils::paths::upload_temp_dir(
                &state.config.server.upload_temp_dir,
                &session.id,
            );
            crate::utils::cleanup_temp_dir(&temp_dir).await;

            match upload_session_repo::delete(&state.db, &session.id).await {
                Ok(()) => {
                    stats.completed_sessions_deleted += 1;
                    if broken_completed {
                        stats.broken_completed_sessions_deleted += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %session.id,
                        "failed to delete expired completed upload session: {e}"
                    );
                }
            }
        }
    }

    Ok(stats)
}

pub async fn reconcile_blob_state(state: &AppState) -> Result<BlobMaintenanceStats> {
    let mut actual_ref_counts = load_actual_blob_ref_counts(state).await?;
    let mut last_blob_id: Option<i64> = None;
    let mut stats = BlobMaintenanceStats::default();

    loop {
        let mut query = FileBlob::find()
            .order_by_asc(file_blob::Column::Id)
            .limit(BLOB_RECONCILE_BATCH_SIZE);
        if let Some(last_blob_id_value) = last_blob_id {
            query = query.filter(file_blob::Column::Id.gt(last_blob_id_value));
        }

        let blobs = query.all(&state.db).await.map_err(AsterError::from)?;
        if blobs.is_empty() {
            break;
        }
        last_blob_id = blobs.last().map(|blob| blob.id);

        for blob in blobs {
            let actual_refs = match actual_ref_counts.remove(&blob.id) {
                Some(count) => i32::try_from(count).map_err(|_| {
                    AsterError::internal_error(format!(
                        "actual ref count overflow for blob {}",
                        blob.id
                    ))
                })?,
                None => 0,
            };

            if actual_refs == 0 {
                if blob.ref_count > 0 {
                    file_repo::decrement_blob_ref_count_by(&state.db, blob.id, blob.ref_count)
                        .await?;
                    stats.ref_count_fixed += 1;
                } else if blob.ref_count < 0 {
                    let mut active: file_blob::ActiveModel = blob.clone().into();
                    active.ref_count = Set(0);
                    active.updated_at = Set(Utc::now());
                    active.update(&state.db).await.map_err(AsterError::from)?;
                    stats.ref_count_fixed += 1;
                }
                if crate::services::file_service::cleanup_unreferenced_blob(state, &blob).await {
                    stats.orphan_blobs_deleted += 1;
                }
                continue;
            }

            if blob.ref_count == actual_refs {
                continue;
            }

            let mut active: file_blob::ActiveModel = blob.into();
            active.ref_count = Set(actual_refs);
            active.updated_at = Set(Utc::now());
            active.update(&state.db).await.map_err(AsterError::from)?;
            stats.ref_count_fixed += 1;
        }
    }

    Ok(stats)
}

async fn load_actual_blob_ref_counts(state: &AppState) -> Result<HashMap<i64, i64>> {
    let mut actual = HashMap::new();

    let file_refs = File::find()
        .select_only()
        .column(file::Column::BlobId)
        .column_as(Expr::col(file::Column::Id).count(), "ref_count")
        .group_by(file::Column::BlobId)
        .into_tuple::<(i64, i64)>()
        .all(&state.db)
        .await
        .map_err(AsterError::from)?;

    for (blob_id, ref_count) in file_refs {
        *actual.entry(blob_id).or_insert(0) += ref_count;
    }

    let version_refs = FileVersion::find()
        .select_only()
        .column(file_version::Column::BlobId)
        .column_as(Expr::col(file_version::Column::Id).count(), "ref_count")
        .group_by(file_version::Column::BlobId)
        .into_tuple::<(i64, i64)>()
        .all(&state.db)
        .await
        .map_err(AsterError::from)?;

    for (blob_id, ref_count) in version_refs {
        *actual.entry(blob_id).or_insert(0) += ref_count;
    }

    Ok(actual)
}

async fn load_tracked_blob_paths(
    state: &AppState,
    candidate_paths: &[String],
) -> Result<HashSet<String>> {
    if candidate_paths.is_empty() {
        return Ok(HashSet::new());
    }

    let paths = FileBlob::find()
        .select_only()
        .column(file_blob::Column::StoragePath)
        .filter(file_blob::Column::StoragePath.is_in(candidate_paths.iter().cloned()))
        .into_tuple::<String>()
        .all(&state.db)
        .await
        .map_err(AsterError::from)?;

    Ok(paths.into_iter().collect())
}

async fn cleanup_broken_completed_session_object(
    state: &AppState,
    session: &upload_session::Model,
    tracked_blob_paths: &HashSet<String>,
) {
    let Some(temp_key) = session.s3_temp_key.as_deref() else {
        return;
    };

    if tracked_blob_paths.contains(temp_key) {
        return;
    }

    let Ok(policy) = policy_repo::find_by_id(&state.db, session.policy_id).await else {
        tracing::warn!(
            session_id = %session.id,
            policy_id = session.policy_id,
            "failed to load storage policy for completed upload session cleanup"
        );
        return;
    };

    let Ok(driver) = state.driver_registry.get_driver(&policy) else {
        tracing::warn!(
            session_id = %session.id,
            policy_id = session.policy_id,
            "failed to resolve storage driver for completed upload session cleanup"
        );
        return;
    };

    if let Some(multipart_id) = session.s3_multipart_id.as_deref() {
        let mut abort_error = None;

        for attempt in 1..=MULTIPART_ABORT_MAX_ATTEMPTS {
            match driver.abort_multipart_upload(temp_key, multipart_id).await {
                Ok(()) => {
                    abort_error = None;
                    break;
                }
                Err(err) => {
                    if attempt == MULTIPART_ABORT_MAX_ATTEMPTS {
                        abort_error = Some(err);
                        break;
                    }

                    let backoff_ms = MULTIPART_ABORT_INITIAL_BACKOFF_MS * (1_u64 << (attempt - 1));
                    tracing::warn!(
                        session_id = %session.id,
                        temp_key = %temp_key,
                        attempt,
                        max_attempts = MULTIPART_ABORT_MAX_ATTEMPTS,
                        backoff_ms,
                        "failed to abort stale multipart upload for completed session, retrying: {err}"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }

        if let Some(e) = abort_error {
            tracing::warn!(
                session_id = %session.id,
                temp_key = %temp_key,
                max_attempts = MULTIPART_ABORT_MAX_ATTEMPTS,
                "failed to abort stale multipart upload for completed session after retries: {e}"
            );

            // 删除对象 key 不能回收仍在进行中的 multipart parts；生产环境仍应配置
            // S3/MinIO 生命周期规则来清理 incomplete multipart uploads。
            if let Err(delete_err) = driver.delete(temp_key).await {
                tracing::warn!(
                    session_id = %session.id,
                    temp_key = %temp_key,
                    "failed to delete stale completed multipart object after abort retries exhausted: {delete_err}"
                );
            }
        }
    } else if let Err(e) = driver.delete(temp_key).await {
        tracing::warn!(
            session_id = %session.id,
            temp_key = %temp_key,
            "failed to delete stale temp object for completed session: {e}"
        );
    }
}
