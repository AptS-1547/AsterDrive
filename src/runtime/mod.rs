//! 运行时模块导出。

pub mod logging;
pub mod panic;
pub mod shutdown;
pub mod startup;
pub mod tasks;

use crate::cache::CacheBackend;
use crate::config::{Config, RuntimeConfig};
use crate::services::{
    mail_service::MailSender, share_service::ShareDownloadRollbackQueue,
    storage_change_service::StorageChangeEvent,
};
use crate::storage::{DriverRegistry, PolicySnapshot};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct PrimaryAppState {
    pub db: DatabaseConnection,
    pub driver_registry: Arc<DriverRegistry>,
    pub runtime_config: Arc<RuntimeConfig>,
    pub policy_snapshot: Arc<PolicySnapshot>,
    pub config: Arc<Config>,
    pub cache: Arc<dyn CacheBackend>,
    pub mail_sender: Arc<dyn MailSender>,
    /// 文件/文件夹变更广播（SSE 消费）
    pub storage_change_tx: tokio::sync::broadcast::Sender<StorageChangeEvent>,
    /// 公开分享下载中途断连时的 download_count 回滚队列
    pub share_download_rollback: ShareDownloadRollbackQueue,
}

#[derive(Clone)]
pub struct FollowerAppState {
    pub db: DatabaseConnection,
    pub driver_registry: Arc<DriverRegistry>,
    pub policy_snapshot: Arc<PolicySnapshot>,
    pub config: Arc<Config>,
    pub cache: Arc<dyn CacheBackend>,
}

pub trait FollowerRuntimeState {
    fn db(&self) -> &DatabaseConnection;
    fn driver_registry(&self) -> &Arc<DriverRegistry>;
    fn policy_snapshot(&self) -> &Arc<PolicySnapshot>;
    fn config(&self) -> &Arc<Config>;
    fn cache(&self) -> &Arc<dyn CacheBackend>;
}

impl PrimaryAppState {
    pub fn follower_view(&self) -> FollowerAppState {
        FollowerAppState::from(self)
    }
}

impl From<&PrimaryAppState> for FollowerAppState {
    fn from(state: &PrimaryAppState) -> Self {
        Self {
            db: state.db.clone(),
            driver_registry: state.driver_registry.clone(),
            policy_snapshot: state.policy_snapshot.clone(),
            config: state.config.clone(),
            cache: state.cache.clone(),
        }
    }
}

impl FollowerRuntimeState for PrimaryAppState {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn driver_registry(&self) -> &Arc<DriverRegistry> {
        &self.driver_registry
    }

    fn policy_snapshot(&self) -> &Arc<PolicySnapshot> {
        &self.policy_snapshot
    }

    fn config(&self) -> &Arc<Config> {
        &self.config
    }

    fn cache(&self) -> &Arc<dyn CacheBackend> {
        &self.cache
    }
}

impl FollowerRuntimeState for FollowerAppState {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn driver_registry(&self) -> &Arc<DriverRegistry> {
        &self.driver_registry
    }

    fn policy_snapshot(&self) -> &Arc<PolicySnapshot> {
        &self.policy_snapshot
    }

    fn config(&self) -> &Arc<Config> {
        &self.config
    }

    fn cache(&self) -> &Arc<dyn CacheBackend> {
        &self.cache
    }
}
