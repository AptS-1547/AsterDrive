//! Strongly typed background task specifications.

use sea_orm::ActiveEnum;
use serde::{Serialize, de::DeserializeOwned};

use crate::entities::background_task;
use crate::errors::{AsterError, Result};
use crate::types::{BackgroundTaskKind, BackgroundTaskStatus};

use super::presentation;
use super::retry::{TaskRetryClass, TaskRetryPolicy, default_retry_class};
use super::types::{
    ArchiveCompressTaskPayload, ArchiveCompressTaskResult, ArchiveExtractTaskPayload,
    ArchiveExtractTaskResult, ArchivePreviewTaskPayload, ArchivePreviewTaskResult,
    BlobMaintenanceTaskPayload, BlobMaintenanceTaskResult, MediaMetadataExtractTaskPayload,
    MediaMetadataExtractTaskResult, RuntimeTaskPayload, RuntimeTaskResult,
    StoragePolicyMigrationTaskPayload, StoragePolicyMigrationTaskResult,
    StoragePolicyTempCleanupTaskPayload, StoragePolicyTempCleanupTaskPayloadInfo,
    StoragePolicyTempCleanupTaskResult, TaskPayload, TaskPresentation, TaskResult,
    ThumbnailGenerateTaskPayload, ThumbnailGenerateTaskResult, TrashPurgeAllTaskPayload,
    TrashPurgeAllTaskResult,
};
use super::{archive, media_metadata, runtime, thumbnail};

pub(super) trait BackgroundTaskSpec {
    type Payload: Serialize + DeserializeOwned + Clone + Send + Sync + 'static;
    type Result: Serialize + DeserializeOwned + Clone + Send + Sync + 'static;

    const KIND: BackgroundTaskKind;

    fn wrap_payload(payload: Self::Payload) -> TaskPayload;

    fn wrap_result(result: Self::Result) -> TaskResult;

    fn retry_class(error: &AsterError) -> TaskRetryClass {
        default_retry_class(error)
    }
}

pub(super) fn serialize_payload<S: BackgroundTaskSpec>(
    payload: &S::Payload,
) -> Result<crate::types::StoredTaskPayload> {
    serde_json::to_string(payload)
        .map(crate::types::StoredTaskPayload)
        .map_err(|error| {
            AsterError::internal_error(format!(
                "serialize {} task payload: {error}",
                S::KIND.to_value()
            ))
        })
}

pub(super) fn serialize_result<S: BackgroundTaskSpec>(
    result: &S::Result,
) -> Result<crate::types::StoredTaskResult> {
    serde_json::to_string(result)
        .map(crate::types::StoredTaskResult)
        .map_err(|error| {
            AsterError::internal_error(format!(
                "serialize {} task result: {error}",
                S::KIND.to_value()
            ))
        })
}

pub(super) trait ErasedBackgroundTaskSpec: Sync {
    fn decode_payload(&self, task: &background_task::Model) -> Result<TaskPayload>;

    fn decode_result(&self, task: &background_task::Model) -> Result<Option<TaskResult>>;

    fn presentation(
        &self,
        payload: &TaskPayload,
        result: Option<&TaskResult>,
        status: BackgroundTaskStatus,
    ) -> Result<Option<TaskPresentation>>;

    fn retry_class(&self, error: &AsterError) -> TaskRetryClass;
}

pub(super) struct TaskSpecAdapter<S>(std::marker::PhantomData<S>);

impl<S> TaskSpecAdapter<S> {
    pub(super) const fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<S> ErasedBackgroundTaskSpec for TaskSpecAdapter<S>
where
    S: BackgroundTaskSpec + Sync,
{
    fn decode_payload(&self, task: &background_task::Model) -> Result<TaskPayload> {
        let payload =
            serde_json::from_str::<S::Payload>(task.payload_json.as_ref()).map_err(|error| {
                AsterError::internal_error(format!(
                    "parse payload for task #{} ({}): {error}",
                    task.id,
                    task.kind.to_value()
                ))
            })?;
        Ok(S::wrap_payload(payload))
    }

    fn decode_result(&self, task: &background_task::Model) -> Result<Option<TaskResult>> {
        let Some(raw) = task.result_json.as_ref() else {
            return Ok(None);
        };
        let result = serde_json::from_str::<S::Result>(raw.as_ref()).map_err(|error| {
            AsterError::internal_error(format!(
                "parse result for task #{} ({}): {error}",
                task.id,
                task.kind.to_value()
            ))
        })?;
        Ok(Some(S::wrap_result(result)))
    }

    fn presentation(
        &self,
        payload: &TaskPayload,
        result: Option<&TaskResult>,
        status: BackgroundTaskStatus,
    ) -> Result<Option<TaskPresentation>> {
        Ok(presentation::build_task_presentation(
            payload, result, status,
        ))
    }

    fn retry_class(&self, error: &AsterError) -> TaskRetryClass {
        S::retry_class(error)
    }
}

macro_rules! define_task_spec {
    (
        $spec:ident,
        $kind:ident,
        $payload:ty,
        $result:ty,
        $payload_variant:ident,
        $result_variant:ident
        $(, retry = $retry:path)?
        $(, payload_wrap = $payload_wrap:expr)?
    ) => {
        pub(super) struct $spec;

        impl BackgroundTaskSpec for $spec {
            type Payload = $payload;
            type Result = $result;

            const KIND: BackgroundTaskKind = BackgroundTaskKind::$kind;

            fn wrap_payload(payload: Self::Payload) -> TaskPayload {
                define_task_spec!(@payload_wrap payload, $payload_variant $(, $payload_wrap)?)
            }

            fn wrap_result(result: Self::Result) -> TaskResult {
                TaskResult::$result_variant(result)
            }

            $(
                fn retry_class(error: &AsterError) -> TaskRetryClass {
                    <$retry>::retry_class(error)
                }
            )?
        }
    };
    (@payload_wrap $payload:ident, $variant:ident) => {
        TaskPayload::$variant($payload)
    };
    (@payload_wrap $payload:ident, $variant:ident, $payload_wrap:expr) => {
        TaskPayload::$variant($payload_wrap($payload))
    };
}

define_task_spec!(
    ArchiveCompressTask,
    ArchiveCompress,
    ArchiveCompressTaskPayload,
    ArchiveCompressTaskResult,
    ArchiveCompress,
    ArchiveCompress,
    retry = archive::ArchiveCompressRetryPolicy
);

define_task_spec!(
    ArchiveExtractTask,
    ArchiveExtract,
    ArchiveExtractTaskPayload,
    ArchiveExtractTaskResult,
    ArchiveExtract,
    ArchiveExtract,
    retry = archive::ArchiveExtractRetryPolicy
);

define_task_spec!(
    ArchivePreviewGenerateTask,
    ArchivePreviewGenerate,
    ArchivePreviewTaskPayload,
    ArchivePreviewTaskResult,
    ArchivePreviewGenerate,
    ArchivePreviewGenerate,
    retry = archive::ArchivePreviewRetryPolicy
);

define_task_spec!(
    ThumbnailGenerateTask,
    ThumbnailGenerate,
    ThumbnailGenerateTaskPayload,
    ThumbnailGenerateTaskResult,
    ThumbnailGenerate,
    ThumbnailGenerate,
    retry = thumbnail::ThumbnailRetryPolicy
);

define_task_spec!(
    MediaMetadataExtractTask,
    MediaMetadataExtract,
    MediaMetadataExtractTaskPayload,
    MediaMetadataExtractTaskResult,
    MediaMetadataExtract,
    MediaMetadataExtract,
    retry = media_metadata::MediaMetadataRetryPolicy
);

define_task_spec!(
    TrashPurgeAllTask,
    TrashPurgeAll,
    TrashPurgeAllTaskPayload,
    TrashPurgeAllTaskResult,
    TrashPurgeAll,
    TrashPurgeAll
);

pub(super) struct StoragePolicyTempCleanupTask;

impl BackgroundTaskSpec for StoragePolicyTempCleanupTask {
    type Payload = StoragePolicyTempCleanupTaskPayload;
    type Result = StoragePolicyTempCleanupTaskResult;

    const KIND: BackgroundTaskKind = BackgroundTaskKind::StoragePolicyTempCleanup;

    fn wrap_payload(payload: Self::Payload) -> TaskPayload {
        TaskPayload::StoragePolicyTempCleanup(StoragePolicyTempCleanupTaskPayloadInfo::from(
            payload,
        ))
    }

    fn wrap_result(result: Self::Result) -> TaskResult {
        TaskResult::StoragePolicyTempCleanup(result)
    }
}

define_task_spec!(
    StoragePolicyMigrationTask,
    StoragePolicyMigration,
    StoragePolicyMigrationTaskPayload,
    StoragePolicyMigrationTaskResult,
    StoragePolicyMigration,
    StoragePolicyMigration
);

define_task_spec!(
    BlobMaintenanceTask,
    BlobMaintenance,
    BlobMaintenanceTaskPayload,
    BlobMaintenanceTaskResult,
    BlobMaintenance,
    BlobMaintenance
);

define_task_spec!(
    SystemRuntimeTask,
    SystemRuntime,
    RuntimeTaskPayload,
    RuntimeTaskResult,
    SystemRuntime,
    SystemRuntime,
    retry = runtime::RuntimeRetryPolicy
);
