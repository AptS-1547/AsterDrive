//! 运行时模块导出。

pub mod logging;
pub mod panic;
pub mod shutdown;
pub mod startup;
pub mod tasks;

use crate::cache::CacheBackend;
use crate::config::{Config, RuntimeConfig};
use crate::services::mail_service::MailSender;
use crate::services::storage_change_service::StorageChangeEvent;
use crate::storage::{DriverRegistry, PolicySnapshot};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub driver_registry: Arc<DriverRegistry>,
    pub runtime_config: Arc<RuntimeConfig>,
    pub policy_snapshot: Arc<PolicySnapshot>,
    pub config: Arc<Config>,
    pub cache: Arc<dyn CacheBackend>,
    pub mail_sender: Arc<dyn MailSender>,
    /// 文件/文件夹变更广播（SSE 消费）
    pub storage_change_tx: tokio::sync::broadcast::Sender<StorageChangeEvent>,
}
