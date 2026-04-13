use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use actix_web::web;
use futures::FutureExt;
use tokio::task::JoinHandle;

use super::AppState;

pub struct BackgroundTasks {
    handles: Vec<JoinHandle<()>>,
}

impl BackgroundTasks {
    fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    fn push(&mut self, handle: JoinHandle<()>) {
        self.handles.push(handle);
    }

    pub async fn shutdown(self) {
        for handle in &self.handles {
            handle.abort();
        }

        for handle in self.handles {
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
    state: web::Data<AppState>,
    task_fn: F,
) -> JoinHandle<()>
where
    I: Fn(&AppState) -> Duration + Send + Sync + 'static,
    F: Fn(web::Data<AppState>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut first_run = true;
        loop {
            if !first_run {
                tokio::time::sleep(interval_fn(state.get_ref())).await;
            }
            first_run = false;

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
    })
}

/// Spawn all periodic background cleanup tasks.
pub fn spawn_background_tasks(state: web::Data<AppState>) -> BackgroundTasks {
    let mut tasks = BackgroundTasks::new();

    tasks.push(spawn_periodic(
        "mail-outbox-dispatch",
        mail_outbox_dispatch_interval,
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
