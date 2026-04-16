mod archive;
mod dispatch;
mod steps;
mod types;

use chrono::{Duration, Utc};
use sea_orm::{DatabaseConnection, Set};
use serde::Serialize;

use crate::api::pagination::OffsetPage;
use crate::config::operations;
use crate::db::repository::background_task_repo;
use crate::entities::background_task;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};
use crate::types::{BackgroundTaskKind, BackgroundTaskStatus, StoredTaskResult};

pub(crate) use archive::{
    create_archive_compress_task_in_scope, create_archive_extract_task_in_scope,
    prepare_archive_download_in_scope, stream_archive_download_in_scope,
};
pub use dispatch::{DispatchStats, cleanup_expired, dispatch_due, drain};
use steps::{initial_task_steps, parse_task_steps_json, serialize_task_steps};
pub use types::{
    ArchiveCompressTaskPayload, ArchiveCompressTaskResult, ArchiveExtractTaskPayload,
    ArchiveExtractTaskResult, CreateArchiveCompressTaskParams, CreateArchiveExtractTaskParams,
    CreateArchiveTaskParams, TaskInfo, TaskPayload, TaskResult, TaskStepInfo, TaskStepStatus,
};
use types::{parse_task_payload_info, parse_task_result_info, serialize_task_payload};

pub(super) const DEFAULT_TASK_RETENTION_HOURS: i64 = 24;
pub(super) const TASK_DISPATCH_BATCH_SIZE: u64 = 8;
pub(super) const TASK_PROCESSING_STALE_SECS: i64 = 60;
pub(super) const TASK_LAST_ERROR_MAX_LEN: usize = 1024;
pub(super) const TASK_STATUS_TEXT_MAX_LEN: usize = 255;
pub(super) const TASK_DRAIN_MAX_ROUNDS: usize = 32;
pub(super) const TASK_CLEANUP_BATCH_SIZE: u64 = 64;

pub(crate) async fn list_tasks_paginated_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<TaskInfo>> {
    workspace_storage_service::require_scope_access(state, scope).await?;

    let limit = limit.clamp(1, operations::task_list_max_limit(&state.runtime_config));
    let (tasks, total) = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            background_task_repo::find_paginated_personal(&state.db, user_id, limit, offset).await?
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            background_task_repo::find_paginated_team(&state.db, team_id, limit, offset).await?
        }
    };

    let mut items = Vec::with_capacity(tasks.len());
    for task in tasks {
        items.push(build_task_info(state, task).await?);
    }

    Ok(OffsetPage::new(items, total, limit, offset))
}

pub(crate) async fn get_task_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    task_id: i64,
) -> Result<TaskInfo> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let task = background_task_repo::find_by_id(&state.db, task_id).await?;
    ensure_task_in_scope(&task, scope)?;
    build_task_info(state, task).await
}

pub(crate) async fn retry_task_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    task_id: i64,
) -> Result<TaskInfo> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let task = background_task_repo::find_by_id(&state.db, task_id).await?;
    ensure_task_in_scope(&task, scope)?;

    if task.status != BackgroundTaskStatus::Failed {
        return Err(AsterError::validation_error(
            "only failed tasks can be retried",
        ));
    }

    cleanup_task_temp_dir_for_task(state, task.id).await?;
    let steps_json = serialize_task_steps(&initial_task_steps(task.kind))?;

    let now = Utc::now();
    if !background_task_repo::reset_for_manual_retry(
        &state.db,
        task.id,
        now,
        Some(steps_json.as_ref()),
    )
    .await?
    {
        return Err(AsterError::internal_error(format!(
            "failed to reset task #{} for retry",
            task.id
        )));
    }

    get_task_in_scope(state, scope, task_id).await
}

async fn build_task_info(_state: &AppState, task: background_task::Model) -> Result<TaskInfo> {
    let progress_percent = if task.progress_total <= 0 {
        if task.status == BackgroundTaskStatus::Succeeded {
            100
        } else {
            0
        }
    } else {
        ((task.progress_current.saturating_mul(100)) / task.progress_total).clamp(0, 100) as i32
    };
    let kind = task.kind;
    let payload = parse_task_payload_info(&task)?;
    let result = parse_task_result_info(&task)?;
    let steps = parse_task_steps_json(task.steps_json.as_ref().map(|raw| raw.as_ref()), kind)?;

    Ok(TaskInfo {
        id: task.id,
        kind,
        status: task.status,
        display_name: task.display_name,
        creator_user_id: task.creator_user_id,
        team_id: task.team_id,
        share_id: task.share_id,
        progress_current: task.progress_current,
        progress_total: task.progress_total,
        progress_percent,
        status_text: task.status_text,
        attempt_count: task.attempt_count,
        max_attempts: task.max_attempts,
        last_error: task.last_error,
        payload,
        result,
        steps,
        can_retry: task.status == BackgroundTaskStatus::Failed,
        started_at: task.started_at,
        finished_at: task.finished_at,
        expires_at: task.expires_at,
        created_at: task.created_at,
        updated_at: task.updated_at,
    })
}

pub(super) async fn create_task_record<T: Serialize>(
    state: &AppState,
    scope: WorkspaceStorageScope,
    kind: BackgroundTaskKind,
    display_name: &str,
    payload: &T,
) -> Result<background_task::Model> {
    let now = Utc::now();
    let payload_json = serialize_task_payload(payload)?;
    let steps_json = serialize_task_steps(&initial_task_steps(kind))?;

    background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(kind),
            status: Set(BackgroundTaskStatus::Pending),
            creator_user_id: Set(Some(scope.actor_user_id())),
            team_id: Set(scope.team_id()),
            share_id: Set(None),
            display_name: Set(display_name.to_string()),
            payload_json: Set(payload_json),
            result_json: Set(None),
            steps_json: Set(Some(steps_json)),
            progress_current: Set(0),
            progress_total: Set(0),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(1),
            next_run_at: Set(now),
            processing_started_at: Set(None),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(task_expiration_from(state, now)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub(super) fn task_scope(task: &background_task::Model) -> Result<WorkspaceStorageScope> {
    let actor_user_id = task.creator_user_id.ok_or_else(|| {
        AsterError::internal_error(format!("task #{} is missing creator_user_id", task.id))
    })?;
    Ok(match task.team_id {
        Some(team_id) => WorkspaceStorageScope::Team {
            team_id,
            actor_user_id,
        },
        None => WorkspaceStorageScope::Personal {
            user_id: actor_user_id,
        },
    })
}

pub(super) async fn mark_task_progress(
    state: &AppState,
    task_id: i64,
    current: i64,
    total: i64,
    status_text: Option<&str>,
    steps: &[TaskStepInfo],
) -> Result<()> {
    update_task_progress_db(&state.db, task_id, current, total, status_text, steps).await
}

pub(super) async fn update_task_progress_db(
    db: &DatabaseConnection,
    task_id: i64,
    current: i64,
    total: i64,
    status_text: Option<&str>,
    steps: &[TaskStepInfo],
) -> Result<()> {
    let status_text = status_text.map(truncate_status_text);
    let steps_json = serialize_task_steps(steps)?;
    if background_task_repo::mark_progress(
        db,
        task_id,
        current,
        total,
        status_text.as_deref(),
        Some(steps_json.as_ref()),
    )
    .await?
    {
        Ok(())
    } else {
        Err(AsterError::internal_error(format!(
            "failed to update background task #{} progress",
            task_id
        )))
    }
}

pub(super) async fn mark_task_succeeded(
    state: &AppState,
    task_id: i64,
    result_json: Option<&StoredTaskResult>,
    current: i64,
    total: i64,
    status_text: Option<&str>,
    steps: &[TaskStepInfo],
) -> Result<()> {
    let now = Utc::now();
    let status_text = status_text.map(truncate_status_text);
    let steps_json = serialize_task_steps(steps)?;
    if background_task_repo::mark_succeeded(
        &state.db,
        task_id,
        result_json.map(AsRef::as_ref),
        Some(steps_json.as_ref()),
        current,
        total,
        status_text.as_deref(),
        now,
        task_expiration_from(state, now),
    )
    .await?
    {
        Ok(())
    } else {
        Err(AsterError::internal_error(format!(
            "failed to mark background task #{} as succeeded",
            task_id
        )))
    }
}

pub(super) async fn prepare_task_temp_dir(state: &AppState, task_id: i64) -> Result<String> {
    cleanup_task_temp_dir_for_task(state, task_id).await?;
    let task_temp_dir = crate::utils::paths::task_temp_dir(&state.config.server.temp_dir, task_id);
    tokio::fs::create_dir_all(&task_temp_dir)
        .await
        .map_err(|error| {
            AsterError::storage_driver_error(format!("create task temp dir: {error}"))
        })?;
    Ok(task_temp_dir)
}

pub(super) async fn cleanup_task_temp_dir_for_task(state: &AppState, task_id: i64) -> Result<()> {
    crate::utils::cleanup_temp_dir(&crate::utils::paths::task_temp_dir(
        &state.config.server.temp_dir,
        task_id,
    ))
    .await;
    Ok(())
}

fn ensure_task_in_scope(task: &background_task::Model, scope: WorkspaceStorageScope) -> Result<()> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            if task.team_id.is_some() {
                return Err(AsterError::auth_forbidden(
                    "task belongs to a team workspace",
                ));
            }
            crate::utils::verify_owner(task.creator_user_id.unwrap_or_default(), user_id, "task")?;
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            if task.team_id != Some(team_id) {
                return Err(AsterError::auth_forbidden("task is outside team workspace"));
            }
        }
    }

    Ok(())
}

pub(super) fn task_expiration_from(
    state: &AppState,
    now: chrono::DateTime<chrono::Utc>,
) -> chrono::DateTime<chrono::Utc> {
    now + Duration::hours(load_task_retention_hours(state))
}

fn load_task_retention_hours(state: &AppState) -> i64 {
    let Some(raw) = state.runtime_config.get("task_retention_hours") else {
        return DEFAULT_TASK_RETENTION_HOURS;
    };
    match raw.parse::<i64>() {
        Ok(hours) if hours > 0 => hours,
        _ => {
            tracing::warn!(
                "invalid task_retention_hours value '{}', using default",
                raw
            );
            DEFAULT_TASK_RETENTION_HOURS
        }
    }
}

pub(super) fn truncate_status_text(value: &str) -> String {
    value.chars().take(TASK_STATUS_TEXT_MAX_LEN).collect()
}

pub(super) fn truncate_error(error: &str) -> String {
    error.chars().take(TASK_LAST_ERROR_MAX_LEN).collect()
}
