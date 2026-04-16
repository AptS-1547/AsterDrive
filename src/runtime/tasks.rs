use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use actix_web::web;
use futures::FutureExt;
use rand::RngExt;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::AppState;

const BACKGROUND_TASK_SHUTDOWN_GRACE: Duration = Duration::from_secs(30);
const MAINTENANCE_CLEANUP_JITTER_CAP: Duration = Duration::from_secs(30);

pub struct BackgroundTasks {
    shutdown_token: CancellationToken,
    handles: Vec<JoinHandle<()>>,
}

impl BackgroundTasks {
    fn new() -> Self {
        Self {
            shutdown_token: CancellationToken::new(),
            handles: Vec::new(),
        }
    }

    fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }

    fn push(&mut self, handle: JoinHandle<()>) {
        self.handles.push(handle);
    }

    pub async fn shutdown(self) {
        let BackgroundTasks {
            shutdown_token,
            handles,
        } = self;
        shutdown_token.cancel();

        let deadline = tokio::time::Instant::now() + BACKGROUND_TASK_SHUTDOWN_GRACE;
        while !handles.is_empty()
            && tokio::time::Instant::now() < deadline
            && handles.iter().any(|handle| !handle.is_finished())
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let mut aborted = 0;
        for handle in &handles {
            if !handle.is_finished() {
                handle.abort();
                aborted += 1;
            }
        }
        if aborted > 0 {
            tracing::warn!(
                aborted,
                grace_secs = BACKGROUND_TASK_SHUTDOWN_GRACE.as_secs(),
                "background tasks did not stop before the shutdown grace period; aborting remaining workers"
            );
        }

        for handle in handles {
            let _ = handle.await;
        }
    }
}

/// Spawn a periodic background task with panic recovery.
///
/// Each iteration sleeps using the latest runtime-configured interval before
/// the next run. Panics are caught inside the loop so one failed iteration
/// does not kill the whole periodic worker.
fn spawn_periodic<F, I, Fut>(
    name: &'static str,
    interval_fn: I,
    jitter_cap: Option<Duration>,
    shutdown_token: CancellationToken,
    state: web::Data<AppState>,
    task_fn: F,
) -> JoinHandle<()>
where
    I: Fn(&AppState) -> Duration + Send + Sync + 'static,
    F: Fn(web::Data<AppState>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        run_periodic_iteration(name, &state, &task_fn).await;

        loop {
            let sleep_duration = periodic_sleep_duration(interval_fn(state.get_ref()), jitter_cap);
            tokio::select! {
                biased;
                _ = shutdown_token.cancelled() => break,
                _ = tokio::time::sleep(sleep_duration) => {}
            }

            if shutdown_token.is_cancelled() {
                break;
            }

            run_periodic_iteration(name, &state, &task_fn).await;
        }
    })
}

async fn run_periodic_iteration<F, Fut>(
    name: &'static str,
    state: &web::Data<AppState>,
    task_fn: &F,
) where
    F: Fn(web::Data<AppState>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let s = state.clone();
    if let Err(panic) = AssertUnwindSafe(task_fn(s)).catch_unwind().await {
        let panic_message = if let Some(message) = panic.downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else {
            "unknown panic payload".to_string()
        };
        tracing::error!("background task '{name}' panicked: {panic_message}");
    }
}

fn periodic_sleep_duration(base_interval: Duration, jitter_cap: Option<Duration>) -> Duration {
    let Some(jitter_cap) = jitter_cap else {
        return base_interval;
    };

    let max_jitter_ms = effective_jitter_cap(base_interval, jitter_cap).as_millis();
    if max_jitter_ms == 0 {
        return base_interval;
    }

    let jitter_ms = rand::rng().random_range(0..=max_jitter_ms.min(u128::from(u64::MAX)) as u64);
    base_interval.saturating_add(Duration::from_millis(jitter_ms))
}

fn effective_jitter_cap(base_interval: Duration, jitter_cap: Duration) -> Duration {
    let bounded_ms = (base_interval.as_millis().min(u128::from(u64::MAX)) as u64) / 10;
    jitter_cap.min(Duration::from_millis(bounded_ms))
}

/// Spawn all periodic background cleanup tasks.
pub fn spawn_background_tasks(state: web::Data<AppState>) -> BackgroundTasks {
    let mut tasks = BackgroundTasks::new();
    let shutdown_token = tasks.shutdown_token();

    tasks.push(spawn_periodic(
        "mail-outbox-dispatch",
        mail_outbox_dispatch_interval,
        None,
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::mail_outbox_service::dispatch_due(&s).await {
                Ok(stats) if stats.claimed > 0 || stats.failed > 0 => {
                    tracing::info!(
                        claimed = stats.claimed,
                        sent = stats.sent,
                        retried = stats.retried,
                        failed = stats.failed,
                        "processed mail outbox batch"
                    );
                }
                Err(error) => tracing::warn!("mail outbox dispatch failed: {error}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "background-task-dispatch",
        background_task_dispatch_interval,
        None,
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::task_service::dispatch_due(&s).await {
                Ok(stats) if stats.claimed > 0 || stats.failed > 0 => {
                    tracing::info!(
                        claimed = stats.claimed,
                        succeeded = stats.succeeded,
                        retried = stats.retried,
                        failed = stats.failed,
                        "processed background task batch"
                    );
                }
                Err(error) => tracing::warn!("background task dispatch failed: {error}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "upload-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            if let Err(e) = crate::services::upload_service::cleanup_expired(&s).await {
                tracing::warn!("upload cleanup failed: {e}");
            }
        },
    ));

    tasks.push(spawn_periodic(
        "completed-upload-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::maintenance_service::cleanup_expired_completed_upload_sessions(
                &s,
            )
            .await
            {
                Ok(stats) if stats.completed_sessions_deleted > 0 => tracing::info!(
                    deleted = stats.completed_sessions_deleted,
                    broken = stats.broken_completed_sessions_deleted,
                    "cleaned up expired completed upload sessions"
                ),
                Err(e) => tracing::warn!("completed upload session cleanup failed: {e}"),
                _ => {}
            }
        },
    ));

    // Full-table blob reconciliation is intentionally less frequent than lightweight cleanups.
    tasks.push(spawn_periodic(
        "blob-reconcile",
        blob_reconcile_interval,
        None,
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::maintenance_service::reconcile_blob_state(&s).await {
                Ok(stats) if stats.ref_count_fixed > 0 || stats.orphan_blobs_deleted > 0 => {
                    tracing::info!(
                        ref_count_fixed = stats.ref_count_fixed,
                        orphan_blobs_deleted = stats.orphan_blobs_deleted,
                        "reconciled blob state"
                    );
                }
                Err(e) => tracing::warn!("blob reconcile failed: {e}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "trash-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            if let Err(e) = crate::services::trash_service::cleanup_expired(&s).await {
                tracing::warn!("trash cleanup failed: {e}");
            }
        },
    ));

    tasks.push(spawn_periodic(
        "team-archive-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::team_service::cleanup_expired_archived_teams(&s).await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {count} expired archived teams")
                }
                Err(e) => tracing::warn!("team archive cleanup failed: {e}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "lock-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::lock_service::cleanup_expired(&s).await {
                Ok(n) if n > 0 => tracing::info!("cleaned up {n} expired locks"),
                Err(e) => tracing::warn!("lock cleanup failed: {e}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "audit-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            if let Err(e) = crate::services::audit_service::cleanup_expired(&s).await {
                tracing::warn!("audit log cleanup failed: {e}");
            }
        },
    ));

    tasks.push(spawn_periodic(
        "task-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token.clone(),
        state.clone(),
        |s| async move {
            match crate::services::task_service::cleanup_expired(&s).await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {count} expired task artifacts")
                }
                Err(e) => tracing::warn!("background task cleanup failed: {e}"),
                _ => {}
            }
        },
    ));

    tasks.push(spawn_periodic(
        "wopi-session-cleanup",
        maintenance_cleanup_interval,
        Some(MAINTENANCE_CLEANUP_JITTER_CAP),
        shutdown_token,
        state,
        |s| async move {
            match crate::services::wopi_service::cleanup_expired(&s).await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {count} expired WOPI sessions")
                }
                Err(e) => tracing::warn!("WOPI session cleanup failed: {e}"),
                _ => {}
            }
        },
    ));

    tasks
}

fn mail_outbox_dispatch_interval(state: &AppState) -> Duration {
    Duration::from_secs(
        crate::config::operations::mail_outbox_dispatch_interval_secs(&state.runtime_config),
    )
}

fn background_task_dispatch_interval(state: &AppState) -> Duration {
    Duration::from_secs(
        crate::config::operations::background_task_dispatch_interval_secs(&state.runtime_config),
    )
}

fn maintenance_cleanup_interval(state: &AppState) -> Duration {
    Duration::from_secs(
        crate::config::operations::maintenance_cleanup_interval_secs(&state.runtime_config),
    )
}

fn blob_reconcile_interval(state: &AppState) -> Duration {
    Duration::from_secs(crate::config::operations::blob_reconcile_interval_secs(
        &state.runtime_config,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_sleep_duration_is_unchanged_without_jitter() {
        let base = Duration::from_secs(5);
        assert_eq!(periodic_sleep_duration(base, None), base);
    }

    #[test]
    fn periodic_sleep_duration_caps_jitter_to_ten_percent_of_interval() {
        let base = Duration::from_secs(5);
        let cap = Duration::from_secs(30);

        for _ in 0..64 {
            let delay = periodic_sleep_duration(base, Some(cap));
            assert!(delay >= base);
            assert!(delay <= base + Duration::from_millis(500));
        }
    }

    #[test]
    fn periodic_sleep_duration_uses_requested_cap_when_it_is_smaller() {
        let base = Duration::from_secs(3600);
        let cap = Duration::from_secs(30);

        for _ in 0..64 {
            let delay = periodic_sleep_duration(base, Some(cap));
            assert!(delay >= base);
            assert!(delay <= base + cap);
        }
    }
}
