use sea_orm::ActiveEnum;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::entities::background_task;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::types::{BackgroundTaskKind, BackgroundTaskStatus, StoredTaskPayload, StoredTaskResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TaskStepStatus {
    Pending,
    Active,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TaskStepInfo {
    pub key: String,
    pub title: String,
    pub status: TaskStepStatus,
    pub progress_current: i64,
    pub progress_total: i64,
    pub detail: Option<String>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateArchiveTaskParams {
    pub file_ids: Vec<i64>,
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateArchiveCompressTaskParams {
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub folder_ids: Vec<i64>,
    pub archive_name: Option<String>,
    pub target_folder_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateArchiveExtractTaskParams {
    pub target_folder_id: Option<i64>,
    pub output_folder_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveCompressTaskPayload {
    pub file_ids: Vec<i64>,
    pub folder_ids: Vec<i64>,
    pub archive_name: String,
    pub target_folder_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveExtractTaskPayload {
    pub file_id: i64,
    pub source_file_name: String,
    pub target_folder_id: Option<i64>,
    pub output_folder_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveCompressTaskResult {
    pub target_file_id: i64,
    pub target_file_name: String,
    pub target_folder_id: Option<i64>,
    pub target_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ArchiveExtractTaskResult {
    pub target_folder_id: i64,
    pub target_folder_name: String,
    pub target_path: String,
    pub extracted_file_count: i64,
    pub extracted_folder_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RuntimeTaskPayload {
    pub task_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ThumbnailGenerateTaskPayload {
    pub blob_id: i64,
    pub blob_hash: String,
    pub source_mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RuntimeTaskResult {
    pub duration_ms: i64,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct ThumbnailGenerateTaskResult {
    pub blob_id: i64,
    pub thumbnail_path: String,
    pub reused_existing_thumbnail: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskPayload {
    ArchiveCompress(ArchiveCompressTaskPayload),
    ArchiveExtract(ArchiveExtractTaskPayload),
    ThumbnailGenerate(ThumbnailGenerateTaskPayload),
    SystemRuntime(RuntimeTaskPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskResult {
    ArchiveCompress(ArchiveCompressTaskResult),
    ArchiveExtract(ArchiveExtractTaskResult),
    ThumbnailGenerate(ThumbnailGenerateTaskResult),
    SystemRuntime(RuntimeTaskResult),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TaskInfo {
    pub id: i64,
    pub kind: BackgroundTaskKind,
    pub status: BackgroundTaskStatus,
    pub display_name: String,
    pub creator_user_id: Option<i64>,
    pub team_id: Option<i64>,
    pub share_id: Option<i64>,
    pub progress_current: i64,
    pub progress_total: i64,
    pub progress_percent: i32,
    pub status_text: Option<String>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub payload: TaskPayload,
    pub result: Option<TaskResult>,
    pub steps: Vec<TaskStepInfo>,
    pub can_retry: bool,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub lease_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub expires_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub(super) fn parse_task_payload<T>(task: &background_task::Model) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(task.payload_json.as_ref()).map_err(|error| {
        AsterError::internal_error(format!(
            "parse payload for task #{} ({}): {error}",
            task.id,
            task.kind.to_value()
        ))
    })
}

pub(super) fn parse_task_payload_info(task: &background_task::Model) -> Result<TaskPayload> {
    match task.kind {
        BackgroundTaskKind::ArchiveCompress => {
            Ok(TaskPayload::ArchiveCompress(parse_task_payload(task)?))
        }
        BackgroundTaskKind::ArchiveExtract => {
            Ok(TaskPayload::ArchiveExtract(parse_task_payload(task)?))
        }
        BackgroundTaskKind::ThumbnailGenerate => {
            Ok(TaskPayload::ThumbnailGenerate(parse_task_payload(task)?))
        }
        BackgroundTaskKind::SystemRuntime => {
            Ok(TaskPayload::SystemRuntime(parse_task_payload(task)?))
        }
    }
}

pub(super) fn parse_task_result_info(task: &background_task::Model) -> Result<Option<TaskResult>> {
    let raw = match task.result_json.as_ref() {
        Some(raw) => raw,
        None => return Ok(None),
    };

    match task.kind {
        BackgroundTaskKind::ArchiveCompress => Ok(Some(TaskResult::ArchiveCompress(
            serde_json::from_str(raw.as_ref()).map_err(|error| {
                AsterError::internal_error(format!(
                    "parse result for task #{} ({}): {error}",
                    task.id,
                    task.kind.to_value()
                ))
            })?,
        ))),
        BackgroundTaskKind::ArchiveExtract => Ok(Some(TaskResult::ArchiveExtract(
            serde_json::from_str(raw.as_ref()).map_err(|error| {
                AsterError::internal_error(format!(
                    "parse result for task #{} ({}): {error}",
                    task.id,
                    task.kind.to_value()
                ))
            })?,
        ))),
        BackgroundTaskKind::ThumbnailGenerate => Ok(Some(TaskResult::ThumbnailGenerate(
            serde_json::from_str(raw.as_ref()).map_err(|error| {
                AsterError::internal_error(format!(
                    "parse result for task #{} ({}): {error}",
                    task.id,
                    task.kind.to_value()
                ))
            })?,
        ))),
        BackgroundTaskKind::SystemRuntime => Ok(Some(TaskResult::SystemRuntime(
            serde_json::from_str(raw.as_ref()).map_err(|error| {
                AsterError::internal_error(format!(
                    "parse result for task #{} ({}): {error}",
                    task.id,
                    task.kind.to_value()
                ))
            })?,
        ))),
    }
}

pub(super) fn serialize_task_payload<T: Serialize>(payload: &T) -> Result<StoredTaskPayload> {
    serde_json::to_string(payload)
        .map(StoredTaskPayload)
        .map_aster_err_ctx("serialize task payload", AsterError::internal_error)
}

pub(super) fn serialize_task_result<T: Serialize>(result: &T) -> Result<StoredTaskResult> {
    serde_json::to_string(result)
        .map(StoredTaskResult)
        .map_aster_err_ctx("serialize task result", AsterError::internal_error)
}
