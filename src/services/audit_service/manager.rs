use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration as StdDuration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::db::repository::audit_log_repo;
use crate::entities::audit_log;
use crate::runtime::PrimaryAppState;
use crate::types::AuditAction;

use super::context::AuditContext;

pub(super) const AUDIT_LOG_QUEUE_CAPACITY: usize = 4096;
pub(super) const AUDIT_LOG_BATCH_SIZE: usize = 100;
const AUDIT_LOG_FLUSH_INTERVAL: StdDuration = StdDuration::from_millis(500);

static GLOBAL_AUDIT_LOG_MANAGER: OnceLock<Arc<AuditLogManager>> = OnceLock::new();

pub(super) struct AuditLogManager {
    db: DatabaseConnection,
    buffer: parking_lot::Mutex<Vec<audit_log::ActiveModel>>,
    flush_lock: Mutex<()>,
    flush_pending: AtomicBool,
    shutdown_token: CancellationToken,
}

struct FlushPendingReset {
    manager: Arc<AuditLogManager>,
}

impl Drop for FlushPendingReset {
    fn drop(&mut self) {
        self.manager.flush_pending.store(false, Ordering::Release);
    }
}

pub fn init_global_audit_log_manager(db: DatabaseConnection) {
    let manager = Arc::new(AuditLogManager::new(db));
    match GLOBAL_AUDIT_LOG_MANAGER.set(manager.clone()) {
        Ok(()) => {
            drop(tokio::spawn(manager.start_background_task()));
        }
        Err(_) => {
            tracing::warn!("global audit log manager is already initialized; ignoring");
        }
    }
}

pub async fn flush_global_audit_log_manager() {
    if let Some(manager) = GLOBAL_AUDIT_LOG_MANAGER.get() {
        manager.flush().await;
    }
}

pub async fn shutdown_global_audit_log_manager() {
    if let Some(manager) = GLOBAL_AUDIT_LOG_MANAGER.get() {
        manager.cancel();
        manager.flush().await;
    }
}

async fn write_audit_model(db: &DatabaseConnection, model: audit_log::ActiveModel) {
    if let Err(e) = audit_log_repo::create(db, model).await {
        tracing::warn!("failed to write audit log: {e}");
    }
}

async fn write_audit_batch(db: &DatabaseConnection, batch: &mut Vec<audit_log::ActiveModel>) {
    if batch.is_empty() {
        return;
    }

    let total = batch.len();
    let mut models = std::mem::take(batch).into_iter();
    loop {
        let chunk = models
            .by_ref()
            .take(AUDIT_LOG_BATCH_SIZE)
            .collect::<Vec<_>>();
        if chunk.is_empty() {
            break;
        }

        let count = chunk.len();
        if let Err(e) = audit_log_repo::create_many(db, chunk).await {
            tracing::warn!(count, total, "failed to write audit log batch: {e}");
        }
    }
}

impl AuditLogManager {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            buffer: parking_lot::Mutex::new(Vec::with_capacity(AUDIT_LOG_BATCH_SIZE)),
            flush_lock: Mutex::new(()),
            flush_pending: AtomicBool::new(false),
            shutdown_token: CancellationToken::new(),
        }
    }

    pub(super) async fn record(self: &Arc<Self>, model: audit_log::ActiveModel) {
        let mut overflow_model = None;
        let should_flush;
        {
            let mut buffer = self.buffer.lock();
            if buffer.len() >= AUDIT_LOG_QUEUE_CAPACITY {
                overflow_model = Some(model);
                should_flush = false;
            } else {
                buffer.push(model);
                should_flush = buffer.len() >= AUDIT_LOG_BATCH_SIZE;
            }
        }

        if let Some(model) = overflow_model {
            tracing::warn!(
                capacity = AUDIT_LOG_QUEUE_CAPACITY,
                "audit log buffer is full; falling back to direct write"
            );
            self.schedule_flush();
            write_audit_model(&self.db, model).await;
            return;
        }

        if should_flush {
            self.schedule_flush();
        }
    }

    fn schedule_flush(self: &Arc<Self>) {
        if self
            .flush_pending
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let manager = Arc::clone(self);
        drop(tokio::spawn(async move {
            let _pending_reset = FlushPendingReset {
                manager: Arc::clone(&manager),
            };
            let _guard = manager.flush_lock.lock().await;
            manager.flush_buffer().await;
        }));
    }

    async fn start_background_task(self: Arc<Self>) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown_token.cancelled() => break,
                _ = tokio::time::sleep(AUDIT_LOG_FLUSH_INTERVAL) => {
                    if let Ok(_guard) = self.flush_lock.try_lock() {
                        self.flush_buffer().await;
                    }
                }
            }
        }
    }

    pub(super) async fn flush(&self) {
        let _guard = self.flush_lock.lock().await;
        self.flush_buffer().await;
    }

    pub(super) fn cancel(&self) {
        self.shutdown_token.cancel();
    }

    async fn flush_buffer(&self) {
        let mut models = {
            let mut buffer = self.buffer.lock();
            if buffer.is_empty() {
                return;
            }
            std::mem::take(&mut *buffer)
        };
        write_audit_batch(&self.db, &mut models).await;
    }
}
pub async fn log(
    state: &PrimaryAppState,
    ctx: &AuditContext,
    action: AuditAction,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
    entity_name: Option<&str>,
    details: Option<serde_json::Value>,
) {
    // 检查运行时配置
    if matches!(
        state.runtime_config.get_bool("audit_log_enabled"),
        Some(false)
    ) {
        return;
    }

    let model = audit_log::ActiveModel {
        id: Default::default(),
        user_id: Set(ctx.user_id),
        action: Set(action),
        entity_type: Set(entity_type.map(|s| s.to_string())),
        entity_id: Set(entity_id),
        entity_name: Set(entity_name.map(|s| s.to_string())),
        details: Set(details.map(|v| v.to_string())),
        ip_address: Set(ctx.ip_address.clone()),
        user_agent: Set(ctx.user_agent.clone()),
        created_at: Set(Utc::now()),
    };

    if let Some(manager) = GLOBAL_AUDIT_LOG_MANAGER.get() {
        manager.record(model).await;
    } else {
        write_audit_model(&state.db, model).await;
    }
}
