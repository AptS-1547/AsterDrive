use super::{BackgroundTaskSpec, TaskLeaseGuard, TaskProcessFuture};
use crate::entities::background_task;
use crate::errors::AsterError;
use crate::runtime::PrimaryAppState;
use crate::services::task_service::{
    dispatch::TaskLane,
    retry::TaskRetryClass,
    steps::TaskStepSpec,
    types::{RuntimeTaskPayload, RuntimeTaskResult, TaskPayload, TaskResult},
};
use crate::types::BackgroundTaskKind;

const NO_STEPS: &[TaskStepSpec] = &[];

pub(crate) struct SystemRuntimeTask;

impl BackgroundTaskSpec for SystemRuntimeTask {
    type Payload = RuntimeTaskPayload;
    type Result = RuntimeTaskResult;

    const KIND: BackgroundTaskKind = BackgroundTaskKind::SystemRuntime;

    fn step_specs() -> &'static [TaskStepSpec] {
        NO_STEPS
    }

    fn lane() -> TaskLane {
        TaskLane::Fallback
    }

    fn max_attempts(_state: &PrimaryAppState) -> i32 {
        1
    }

    fn wrap_payload(payload: Self::Payload) -> TaskPayload {
        TaskPayload::SystemRuntime(payload)
    }

    fn wrap_result(result: Self::Result) -> TaskResult {
        TaskResult::SystemRuntime(result)
    }

    fn process<'a>(
        _state: &'a PrimaryAppState,
        task: &'a background_task::Model,
        _lease_guard: TaskLeaseGuard,
    ) -> TaskProcessFuture<'a> {
        Box::pin(async move {
            Err(AsterError::internal_error(format!(
                "system runtime task #{} should not be dispatched",
                task.id
            )))
        })
    }

    fn retry_class(_error: &AsterError) -> TaskRetryClass {
        TaskRetryClass::Never
    }
}
