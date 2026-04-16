use chrono::{Duration, Utc};
use sea_orm::ActiveEnum;

use crate::db::repository::background_task_repo;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::types::BackgroundTaskKind;

use super::archive;
use super::steps::{mark_active_step_failed, parse_task_steps_json, serialize_task_steps};
use super::{
    TASK_CLEANUP_BATCH_SIZE, TASK_DISPATCH_BATCH_SIZE, TASK_DRAIN_MAX_ROUNDS,
    TASK_PROCESSING_STALE_SECS, task_expiration_from, truncate_error,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DispatchStats {
    pub claimed: usize,
    pub succeeded: usize,
    pub retried: usize,
    pub failed: usize,
}

pub async fn dispatch_due(state: &AppState) -> Result<DispatchStats> {
    let now = Utc::now();
    let stale_before = now - Duration::seconds(TASK_PROCESSING_STALE_SECS);
    let due = background_task_repo::list_claimable(
        &state.db,
        now,
        stale_before,
        TASK_DISPATCH_BATCH_SIZE,
    )
    .await?;
    let mut stats = DispatchStats::default();

    for task in due {
        let claimed_at = Utc::now();
        if !background_task_repo::try_claim(&state.db, task.id, claimed_at, stale_before).await? {
            continue;
        }

        stats.claimed += 1;
        match process_task(state, &task).await {
            Ok(()) => stats.succeeded += 1,
            Err(error) => {
                let attempt_count = task.attempt_count + 1;
                let error_message = truncate_error(&error.to_string());
                let failed_steps_json =
                    build_failed_task_steps_json(state, task.id, task.kind, &error_message).await;
                if attempt_count >= task.max_attempts {
                    if background_task_repo::mark_failed(
                        &state.db,
                        task.id,
                        attempt_count,
                        &error_message,
                        Utc::now(),
                        task_expiration_from(state, Utc::now()),
                        failed_steps_json.as_deref(),
                    )
                    .await?
                    {
                        stats.failed += 1;
                    }
                    tracing::warn!(
                        task_id = task.id,
                        kind = %task.kind.to_value(),
                        attempt_count,
                        error = %error_message,
                        "background task permanently failed"
                    );
                } else {
                    let retry_at = Utc::now() + Duration::seconds(retry_delay_secs(attempt_count));
                    if background_task_repo::mark_retry(
                        &state.db,
                        task.id,
                        attempt_count,
                        retry_at,
                        &error_message,
                        failed_steps_json.as_deref(),
                    )
                    .await?
                    {
                        stats.retried += 1;
                    }
                    tracing::warn!(
                        task_id = task.id,
                        kind = %task.kind.to_value(),
                        attempt_count,
                        retry_at = %retry_at,
                        error = %error_message,
                        "background task failed; scheduled retry"
                    );
                }
            }
        }
    }

    Ok(stats)
}

async fn build_failed_task_steps_json(
    state: &AppState,
    task_id: i64,
    kind: BackgroundTaskKind,
    error_message: &str,
) -> Option<String> {
    let latest = background_task_repo::find_by_id(&state.db, task_id)
        .await
        .ok()?;
    let mut steps =
        parse_task_steps_json(latest.steps_json.as_ref().map(|raw| raw.as_ref()), kind).ok()?;
    if steps.is_empty() {
        return None;
    }
    mark_active_step_failed(&mut steps, Some(error_message));
    serialize_task_steps(&steps).ok().map(Into::into)
}

pub async fn drain(state: &AppState) -> Result<DispatchStats> {
    let mut total = DispatchStats::default();

    for _ in 0..TASK_DRAIN_MAX_ROUNDS {
        let stats = dispatch_due(state).await?;
        let claimed = stats.claimed;
        total.claimed += stats.claimed;
        total.succeeded += stats.succeeded;
        total.retried += stats.retried;
        total.failed += stats.failed;
        if claimed == 0 {
            break;
        }
    }

    Ok(total)
}

pub async fn cleanup_expired(state: &AppState) -> Result<u64> {
    let now = Utc::now();
    let expired_tasks =
        background_task_repo::list_expired_terminal(&state.db, now, TASK_CLEANUP_BATCH_SIZE)
            .await?;
    for task in &expired_tasks {
        crate::utils::cleanup_temp_dir(&crate::utils::paths::task_temp_dir(
            &state.config.server.temp_dir,
            task.id,
        ))
        .await;
    }
    let removed_tasks = background_task_repo::delete_many(
        &state.db,
        &expired_tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
    )
    .await?;

    Ok(removed_tasks)
}

async fn process_task(
    state: &AppState,
    task: &crate::entities::background_task::Model,
) -> Result<()> {
    match task.kind {
        BackgroundTaskKind::ArchiveCompress => {
            archive::process_archive_compress_task(state, task).await
        }
        BackgroundTaskKind::ArchiveExtract => {
            archive::process_archive_extract_task(state, task).await
        }
    }
}

fn retry_delay_secs(attempt_count: i32) -> i64 {
    match attempt_count {
        1 => 5,
        2 => 15,
        3 => 60,
        _ => 300,
    }
}
