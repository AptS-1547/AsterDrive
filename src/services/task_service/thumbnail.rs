//! 后台任务服务子模块：`thumbnail`。

use chrono::Utc;
use sea_orm::Set;

use crate::db::repository::background_task_repo;
use crate::entities::{background_task, file_blob};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::thumbnail_service;
use crate::types::{BackgroundTaskKind, BackgroundTaskStatus};

use super::steps::{
    TASK_STEP_INSPECT_SOURCE, TASK_STEP_PERSIST_THUMBNAIL, TASK_STEP_RENDER_THUMBNAIL,
    TASK_STEP_WAITING, initial_task_steps, parse_task_steps_json, serialize_task_steps,
    set_task_step_active, set_task_step_succeeded,
};
use super::types::{
    ThumbnailGenerateTaskPayload, ThumbnailGenerateTaskResult, parse_task_payload,
    serialize_task_payload, serialize_task_result,
};
use super::{
    TaskLeaseGuard, configured_task_max_attempts, mark_task_progress, mark_task_succeeded,
    task_expiration_from,
};

pub(crate) async fn ensure_thumbnail_task(
    state: &AppState,
    blob: &file_blob::Model,
    source_mime_type: &str,
) -> Result<()> {
    let display_name = thumbnail_task_display_name(blob.id);
    if let Some(existing) = background_task_repo::find_latest_by_kind_and_display_name(
        &state.db,
        BackgroundTaskKind::ThumbnailGenerate,
        &display_name,
    )
    .await?
    {
        match existing.status {
            BackgroundTaskStatus::Pending
            | BackgroundTaskStatus::Processing
            | BackgroundTaskStatus::Retry => return Ok(()),
            BackgroundTaskStatus::Failed => {
                return Err(AsterError::thumbnail_generation_failed(
                    existing
                        .last_error
                        .unwrap_or_else(|| "thumbnail generation failed".to_string()),
                ));
            }
            BackgroundTaskStatus::Succeeded | BackgroundTaskStatus::Canceled => {}
        }
    }

    let now = Utc::now();
    let payload = ThumbnailGenerateTaskPayload {
        blob_id: blob.id,
        blob_hash: blob.hash.clone(),
        source_mime_type: source_mime_type.to_string(),
    };
    let payload_json = serialize_task_payload(&payload)?;
    let steps_json =
        serialize_task_steps(&initial_task_steps(BackgroundTaskKind::ThumbnailGenerate))?;
    background_task_repo::create(
        &state.db,
        background_task::ActiveModel {
            kind: Set(BackgroundTaskKind::ThumbnailGenerate),
            status: Set(BackgroundTaskStatus::Pending),
            creator_user_id: Set(None),
            team_id: Set(None),
            share_id: Set(None),
            display_name: Set(display_name),
            payload_json: Set(payload_json),
            result_json: Set(None),
            steps_json: Set(Some(steps_json)),
            progress_current: Set(0),
            progress_total: Set(4),
            status_text: Set(None),
            attempt_count: Set(0),
            max_attempts: Set(configured_task_max_attempts(
                state,
                BackgroundTaskKind::ThumbnailGenerate,
            )),
            next_run_at: Set(now),
            processing_token: Set(0),
            processing_started_at: Set(None),
            last_heartbeat_at: Set(None),
            lease_expires_at: Set(None),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            expires_at: Set(task_expiration_from(state, now)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;

    let state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = super::dispatch_due(&state).await {
            tracing::warn!("thumbnail task eager dispatch failed: {error}");
        }
    });

    Ok(())
}

pub(super) async fn process_thumbnail_generate_task(
    state: &AppState,
    task: &background_task::Model,
    lease_guard: TaskLeaseGuard,
) -> Result<()> {
    let payload: ThumbnailGenerateTaskPayload = parse_task_payload(task)?;
    let mut steps =
        parse_task_steps_json(task.steps_json.as_ref().map(|raw| raw.as_ref()), task.kind)?;
    set_task_step_succeeded(
        &mut steps,
        TASK_STEP_WAITING,
        Some("Worker claimed task"),
        Some((1, 4)),
    )?;
    set_task_step_active(
        &mut steps,
        TASK_STEP_INSPECT_SOURCE,
        Some("Loading source blob"),
        Some((1, 4)),
    )?;
    mark_task_progress(
        state,
        &lease_guard,
        1,
        4,
        Some("Loading source blob"),
        &steps,
    )
    .await?;

    let blob =
        crate::db::repository::file_repo::find_blob_by_id(&state.db, payload.blob_id).await?;
    lease_guard.ensure_active()?;
    set_task_step_succeeded(
        &mut steps,
        TASK_STEP_INSPECT_SOURCE,
        Some("Source blob is ready"),
        Some((2, 4)),
    )?;
    set_task_step_active(
        &mut steps,
        TASK_STEP_RENDER_THUMBNAIL,
        Some("Generating thumbnail"),
        Some((2, 4)),
    )?;
    mark_task_progress(
        state,
        &lease_guard,
        2,
        4,
        Some("Generating thumbnail"),
        &steps,
    )
    .await?;

    let (thumbnail_path, reused_existing_thumbnail) =
        thumbnail_service::generate_and_store(state, &blob).await?;
    lease_guard.ensure_active()?;

    if reused_existing_thumbnail {
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_RENDER_THUMBNAIL,
            Some("Existing thumbnail reused"),
            Some((3, 4)),
        )?;
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_PERSIST_THUMBNAIL,
            Some("Existing thumbnail reused"),
            Some((4, 4)),
        )?;
    } else {
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_RENDER_THUMBNAIL,
            Some("Thumbnail rendered"),
            Some((3, 4)),
        )?;
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_PERSIST_THUMBNAIL,
            Some("Thumbnail stored"),
            Some((4, 4)),
        )?;
    }

    let result_json = serialize_task_result(&ThumbnailGenerateTaskResult {
        blob_id: blob.id,
        thumbnail_path: thumbnail_path.clone(),
        reused_existing_thumbnail,
    })?;
    let status_text = if reused_existing_thumbnail {
        "Thumbnail already available"
    } else {
        "Thumbnail ready"
    };
    mark_task_succeeded(
        state,
        &lease_guard,
        Some(&result_json),
        4,
        4,
        Some(status_text),
        &steps,
    )
    .await
}

fn thumbnail_task_display_name(blob_id: i64) -> String {
    format!("Generate thumbnail for blob #{blob_id}")
}
