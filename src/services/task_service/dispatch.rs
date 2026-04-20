//! 后台任务 dispatcher。
//!
//! 这层负责从数据库认领可执行任务、按并发上限驱动执行，并在 lease 丢失时
//! 阻止旧 worker 继续把状态写回数据库。

use std::future::Future;

use chrono::{Duration, Utc};
use futures::stream::{self, StreamExt};
use sea_orm::ActiveEnum;
use tokio::time::MissedTickBehavior;

use crate::config::operations;
use crate::db::repository::background_task_repo;
use crate::entities::background_task;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::types::{BackgroundTaskKind, BackgroundTaskStatus};

use super::archive;
use super::steps::{mark_active_step_failed, parse_task_steps_json, serialize_task_steps};
use super::thumbnail;
use super::{
    TASK_DRAIN_MAX_ROUNDS, TASK_HEARTBEAT_INTERVAL_SECS, TASK_PROCESSING_STALE_SECS, TaskLease,
    TaskLeaseGuard, is_task_lease_lost, is_task_lease_renewal_timed_out, task_expiration_from,
    task_lease_expires_at, truncate_error,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DispatchStats {
    pub claimed: usize,
    pub succeeded: usize,
    pub retried: usize,
    pub failed: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct TaskDispatchOutcome {
    succeeded: usize,
    retried: usize,
    failed: usize,
}

pub async fn dispatch_due(state: &PrimaryAppState) -> Result<DispatchStats> {
    let now = Utc::now();
    let stale_before = now - Duration::seconds(TASK_PROCESSING_STALE_SECS);
    let concurrency = operations::background_task_max_concurrency(&state.runtime_config);
    let due = background_task_repo::list_claimable(
        &state.db,
        now,
        stale_before,
        u64::try_from(concurrency).unwrap_or_else(|_| {
            tracing::warn!(
                concurrency,
                "background task concurrency exceeds u64; falling back to a single claimed task"
            );
            1
        }),
    )
    .await?;
    let mut stats = DispatchStats::default();
    let mut claimed_tasks = Vec::with_capacity(due.len());

    for task in due {
        let task_id = task.id;
        let claimed_at = Utc::now();
        let next_processing_token = task.processing_token.checked_add(1).ok_or_else(|| {
            AsterError::internal_error("background task processing token overflow")
        })?;
        // `try_claim` 是真正的 CAS 关口：即使多台进程同时看到同一条 due task，
        // 也只有 token/状态仍匹配的那个 worker 能把它认领成功。
        if !background_task_repo::try_claim(
            &state.db,
            task_id,
            task.processing_token,
            claimed_at,
            stale_before,
            next_processing_token,
            task_lease_expires_at(claimed_at),
        )
        .await?
        {
            continue;
        }

        stats.claimed += 1;
        claimed_tasks.push((task, TaskLease::new(task_id, next_processing_token)));
    }

    // 先把认领结果固定下来，再按并发上限执行，避免边迭代边改库时混淆统计口径。
    let results = run_with_concurrency_limit(claimed_tasks, concurrency, |(task, lease)| {
        let state = state.clone();
        async move { process_claimed_task(&state, task, lease).await }
    })
    .await;
    let mut first_error = None;

    for result in results {
        match result {
            Ok(outcome) => {
                stats.succeeded += outcome.succeeded;
                stats.retried += outcome.retried;
                stats.failed += outcome.failed;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(stats)
}

async fn process_claimed_task(
    state: &PrimaryAppState,
    task: background_task::Model,
    lease: TaskLease,
) -> Result<TaskDispatchOutcome> {
    let mut heartbeat =
        tokio::time::interval(std::time::Duration::from_secs(TASK_HEARTBEAT_INTERVAL_SECS));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let lease_guard = TaskLeaseGuard::new(lease);

    // 外层 select! 同时盯两件事：
    // 1. 真实业务流程是否完成；
    // 2. heartbeat 是否还能继续证明“我还是当前合法 worker”。
    //
    // 注意这里只能取消 async 外壳。真正耗时的压缩/解压是在 spawn_blocking 里，
    // 所以业务代码内部也必须周期性检查 lease guard，才能把旧 worker 真正停下来。
    let process_future = process_task(state, &task, lease_guard.clone());
    tokio::pin!(process_future);

    let task_result = loop {
        tokio::select! {
            biased;
            result = &mut process_future => break result,
            _ = heartbeat.tick() => {
                // 心跳写入返回 Err 时不能直接把 worker 判死，否则一次瞬时 DB 抖动
                // 就会在 60 秒后把长任务误判成 stale 并触发二次认领。
                match evaluate_heartbeat_result(
                    &lease_guard,
                    {
                        let now = Utc::now();
                        background_task_repo::touch_heartbeat(
                            &state.db,
                            task.id,
                            lease.processing_token,
                            now,
                            task_lease_expires_at(now),
                        )
                        .await
                    },
                ) {
                    Ok(()) => {}
                    Err(error) => break Err(error),
                }
            }
        }
    };

    match task_result {
        Ok(()) => Ok(TaskDispatchOutcome {
            succeeded: 1,
            ..Default::default()
        }),
        Err(error) => {
            // lease 丢失 / 续约超时代表“这条执行流已经过期”，不是业务失败。
            // 这时不要再把任务改成 Failed/Retry，否则旧 worker 可能覆盖新 lease 的结果。
            if is_task_lease_lost(&error) || is_task_lease_renewal_timed_out(&error) {
                tracing::info!(
                    task_id = task.id,
                    processing_token = lease.processing_token,
                    "background task worker stopped because its lease is no longer active; skipping stale completion"
                );
                return Ok(TaskDispatchOutcome::default());
            }
            let attempt_count = task.attempt_count + 1;
            let error_message = truncate_error(&error.to_string());
            let failed_steps_json =
                build_failed_task_steps_json(state, task.id, task.kind, &error_message).await;
            let should_retry = should_retry_task_error(task.kind, &error);
            if attempt_count >= task.max_attempts || !should_retry {
                let finished_at = Utc::now();
                let failed = background_task_repo::mark_failed(
                    &state.db,
                    background_task_repo::TaskFailureUpdate {
                        id: task.id,
                        processing_token: lease.processing_token,
                        attempt_count,
                        last_error: &error_message,
                        finished_at,
                        expires_at: task_expiration_from(state, finished_at),
                        steps_json: failed_steps_json.as_deref(),
                    },
                )
                .await?;
                if !failed {
                    tracing::info!(
                        task_id = task.id,
                        processing_token = lease.processing_token,
                        "background task lease moved before failure state update; ignoring stale worker"
                    );
                    return Ok(TaskDispatchOutcome::default());
                }
                tracing::warn!(
                    task_id = task.id,
                    kind = %task.kind.to_value(),
                    attempt_count,
                    error = %error_message,
                    "background task permanently failed"
                );
                Ok(TaskDispatchOutcome {
                    failed: usize::from(failed),
                    ..Default::default()
                })
            } else {
                let retry_at = Utc::now() + Duration::seconds(retry_delay_secs(attempt_count));
                let retried = background_task_repo::mark_retry(
                    &state.db,
                    task.id,
                    lease.processing_token,
                    attempt_count,
                    retry_at,
                    &error_message,
                    failed_steps_json.as_deref(),
                )
                .await?;
                if !retried {
                    tracing::info!(
                        task_id = task.id,
                        processing_token = lease.processing_token,
                        "background task lease moved before retry state update; ignoring stale worker"
                    );
                    return Ok(TaskDispatchOutcome::default());
                }
                tracing::warn!(
                    task_id = task.id,
                    kind = %task.kind.to_value(),
                    attempt_count,
                    retry_at = %retry_at,
                    error = %error_message,
                    "background task failed; scheduled retry"
                );
                Ok(TaskDispatchOutcome {
                    retried: usize::from(retried),
                    ..Default::default()
                })
            }
        }
    }
}

fn evaluate_heartbeat_result(lease_guard: &TaskLeaseGuard, result: Result<bool>) -> Result<()> {
    let lease = lease_guard.lease();
    match result {
        Ok(true) => {
            lease_guard.record_renewed();
            Ok(())
        }
        Ok(false) => {
            // false 不是数据库故障，而是条件更新没命中：
            // 一般表示 status/token 已经不是当前 worker 的了，任务已被别的 lease 接管。
            tracing::info!(
                task_id = lease.task_id,
                processing_token = lease.processing_token,
                "background task lease lost; stopping outdated worker"
            );
            Err(lease_guard.mark_lost())
        }
        Err(error) => {
            // 这里只记录并等待下一轮 heartbeat 重试；真正要停 worker 的信号只能是
            // token 不再匹配，或者连续太久没有任何成功续约。
            //
            // 也就是说，瞬时 DB 抖动不会立刻触发二次认领；只有抖动长到超过
            // renewal_timeout，lease guard 才会把当前 worker 判定为不再安全继续执行。
            tracing::warn!(
                task_id = lease.task_id,
                processing_token = lease.processing_token,
                error = %error,
                "background task heartbeat update failed; continuing and retrying next heartbeat"
            );
            lease_guard.ensure_active()
        }
    }
}

async fn build_failed_task_steps_json(
    state: &PrimaryAppState,
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

pub async fn drain(state: &PrimaryAppState) -> Result<DispatchStats> {
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

pub async fn cleanup_expired(state: &PrimaryAppState) -> Result<u64> {
    let now = Utc::now();
    let tasks_root = crate::utils::paths::temp_file_path(&state.config.server.temp_dir, "tasks");
    let mut entries = match tokio::fs::read_dir(&tasks_root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(AsterError::storage_driver_error(format!(
                "read task temp root {tasks_root}: {error}"
            )));
        }
    };
    let mut cleaned = 0;

    while let Some(entry) = entries.next_entry().await.map_err(|error| {
        AsterError::storage_driver_error(format!("iterate task temp root {tasks_root}: {error}"))
    })? {
        let path = entry.path();
        let path_display = path.to_string_lossy().to_string();
        let file_type = entry.file_type().await.map_err(|error| {
            AsterError::storage_driver_error(format!(
                "read task temp entry type {path_display}: {error}"
            ))
        })?;
        if !file_type.is_dir() {
            continue;
        }

        let dir_name = entry.file_name();
        let Some(dir_name) = dir_name.to_str() else {
            tracing::warn!(path = %path_display, "skipping task temp dir with non-utf8 name");
            continue;
        };
        let Ok(task_id) = dir_name.parse::<i64>() else {
            tracing::warn!(path = %path_display, "skipping task temp dir with invalid task id");
            continue;
        };

        // 这里只删“产物目录”，不删 background_task 记录：
        // - 终态且 expires_at 已到的任务：删 temp 目录，保留历史行；
        // - 数据库里已经没有任务行的孤儿目录：直接删，避免长期泄露磁盘。
        let should_cleanup = match background_task_repo::find_by_id(&state.db, task_id).await {
            Ok(task) => {
                task.expires_at <= now
                    && matches!(
                        task.status,
                        BackgroundTaskStatus::Succeeded
                            | BackgroundTaskStatus::Failed
                            | BackgroundTaskStatus::Canceled
                    )
            }
            Err(AsterError::RecordNotFound(_)) => {
                tracing::warn!(
                    task_id,
                    path = %path_display,
                    "cleaning orphaned task temp dir without task record"
                );
                true
            }
            Err(error) => return Err(error),
        };
        if !should_cleanup {
            continue;
        }

        crate::utils::cleanup_temp_dir(&path_display).await;
        let still_exists = tokio::fs::try_exists(&path).await.map_err(|error| {
            AsterError::storage_driver_error(format!(
                "verify task temp dir cleanup {path_display}: {error}"
            ))
        })?;
        if still_exists {
            tracing::warn!(
                task_id,
                path = %path_display,
                "task temp dir still exists after cleanup attempt"
            );
            continue;
        }

        cleaned += 1;
    }

    Ok(cleaned)
}

async fn process_task(
    state: &PrimaryAppState,
    task: &background_task::Model,
    lease_guard: TaskLeaseGuard,
) -> Result<()> {
    match task.kind {
        BackgroundTaskKind::ArchiveCompress => {
            archive::process_archive_compress_task(state, task, lease_guard).await
        }
        BackgroundTaskKind::ArchiveExtract => {
            archive::process_archive_extract_task(state, task, lease_guard).await
        }
        BackgroundTaskKind::ThumbnailGenerate => {
            thumbnail::process_thumbnail_generate_task(state, task, lease_guard).await
        }
        BackgroundTaskKind::SystemRuntime => Err(crate::errors::AsterError::internal_error(
            format!("system runtime task #{} should not be dispatched", task.id),
        )),
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

fn should_retry_task_error(kind: BackgroundTaskKind, error: &AsterError) -> bool {
    match kind {
        BackgroundTaskKind::ThumbnailGenerate => matches!(
            error,
            AsterError::DatabaseConnection(_)
                | AsterError::DatabaseOperation(_)
                | AsterError::StorageDriverError(_)
        ),
        BackgroundTaskKind::ArchiveCompress
        | BackgroundTaskKind::ArchiveExtract
        | BackgroundTaskKind::SystemRuntime => true,
    }
}

async fn run_with_concurrency_limit<T, O, F, Fut>(items: Vec<T>, limit: usize, handler: F) -> Vec<O>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = O>,
{
    stream::iter(items.into_iter().map(handler))
        .buffer_unordered(limit.max(1))
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use tokio::time::{Duration, sleep};

    use crate::errors::AsterError;

    use super::{evaluate_heartbeat_result, run_with_concurrency_limit};
    use crate::services::task_service::{
        TaskLease, TaskLeaseGuard, is_task_lease_lost, is_task_lease_renewal_timed_out,
    };

    #[tokio::test]
    async fn run_with_concurrency_limit_caps_parallelism() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));

        let mut results = run_with_concurrency_limit(vec![1, 2, 3, 4, 5], 2, {
            let in_flight = in_flight.clone();
            let max_in_flight = max_in_flight.clone();
            move |value| {
                let in_flight = in_flight.clone();
                let max_in_flight = max_in_flight.clone();
                async move {
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    if let Err(e) =
                        max_in_flight.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |existing| {
                            Some(existing.max(current))
                        })
                    {
                        tracing::trace!("max_in_flight fetch_update failed: {e}");
                    }
                    sleep(Duration::from_millis(20)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    value * 2
                }
            }
        })
        .await;

        results.sort_unstable();
        assert_eq!(results, vec![2, 4, 6, 8, 10]);
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);
        assert_eq!(in_flight.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn evaluate_heartbeat_result_keeps_retrying_after_transient_error() {
        let lease = TaskLease::new(42, 7);
        let lease_guard = TaskLeaseGuard::with_renewal_timeout(lease, Duration::from_secs(60));
        let result =
            evaluate_heartbeat_result(&lease_guard, Err(AsterError::database_operation("boom")));
        assert!(result.is_ok());
    }

    #[test]
    fn evaluate_heartbeat_result_reports_lease_loss_when_claim_replaced() {
        let lease = TaskLease::new(42, 7);
        let lease_guard = TaskLeaseGuard::with_renewal_timeout(lease, Duration::from_secs(60));
        let error = evaluate_heartbeat_result(&lease_guard, Ok(false))
            .expect_err("heartbeat mismatch should report lease loss");
        assert!(is_task_lease_lost(&error));
    }

    #[tokio::test]
    async fn evaluate_heartbeat_result_stops_worker_after_renewal_timeout() {
        let lease = TaskLease::new(42, 7);
        let lease_guard = TaskLeaseGuard::with_renewal_timeout(lease, Duration::from_millis(20));
        sleep(Duration::from_millis(30)).await;

        let error =
            evaluate_heartbeat_result(&lease_guard, Err(AsterError::database_operation("boom")))
                .expect_err("expired renewal window should stop the worker");
        assert!(is_task_lease_renewal_timed_out(&error));
    }
}
