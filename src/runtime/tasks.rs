use std::future::Future;
use std::time::Duration;

use actix_web::web;

use super::AppState;

/// Spawn a periodic background task with panic recovery.
///
/// Each iteration runs in a child `tokio::spawn` so that a panic is caught
/// by the `JoinHandle` instead of killing the loop. On panic the error is
/// logged and the next interval fires normally.
fn spawn_periodic<F, Fut>(
    name: &'static str,
    interval: Duration,
    state: web::Data<AppState>,
    task_fn: F,
) where
    F: Fn(web::Data<AppState>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let s = state.clone();
            if let Err(e) = tokio::spawn(task_fn(s)).await {
                tracing::error!("background task '{name}' panicked: {e}");
            }
        }
    });
}

/// Spawn all periodic background cleanup tasks.
pub fn spawn_background_tasks(state: web::Data<AppState>) {
    let mail_dispatch_interval = Duration::from_secs(
        crate::services::mail_outbox_service::MAIL_OUTBOX_DISPATCH_INTERVAL_SECS,
    );
    let hourly = Duration::from_secs(3600);
    let every_six_hours = Duration::from_secs(6 * 3600);

    spawn_periodic(
        "mail-outbox-dispatch",
        mail_dispatch_interval,
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
    );

    spawn_periodic("upload-cleanup", hourly, state.clone(), |s| async move {
        if let Err(e) = crate::services::upload_service::cleanup_expired(&s).await {
            tracing::warn!("upload cleanup failed: {e}");
        }
    });

    spawn_periodic(
        "completed-upload-cleanup",
        hourly,
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
    );

    // Full-table blob reconciliation is intentionally less frequent than lightweight cleanups.
    spawn_periodic(
        "blob-reconcile",
        every_six_hours,
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
    );

    spawn_periodic("trash-cleanup", hourly, state.clone(), |s| async move {
        if let Err(e) = crate::services::trash_service::cleanup_expired(&s).await {
            tracing::warn!("trash cleanup failed: {e}");
        }
    });

    spawn_periodic(
        "team-archive-cleanup",
        hourly,
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
    );

    spawn_periodic("lock-cleanup", hourly, state.clone(), |s| async move {
        match crate::services::lock_service::cleanup_expired(&s).await {
            Ok(n) if n > 0 => tracing::info!("cleaned up {n} expired locks"),
            Err(e) => tracing::warn!("lock cleanup failed: {e}"),
            _ => {}
        }
    });

    spawn_periodic("audit-cleanup", hourly, state, |s| async move {
        if let Err(e) = crate::services::audit_service::cleanup_expired(&s).await {
            tracing::warn!("audit log cleanup failed: {e}");
        }
    });
}
