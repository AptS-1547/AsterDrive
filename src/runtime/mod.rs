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
use actix_web::web;
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

pub trait SharedRuntimeState {
    fn db(&self) -> &DatabaseConnection;
    fn driver_registry(&self) -> &Arc<DriverRegistry>;
    fn policy_snapshot(&self) -> &Arc<PolicySnapshot>;
    fn config(&self) -> &Arc<Config>;
    fn cache(&self) -> &Arc<dyn CacheBackend>;
}

pub trait PrimaryRuntimeState: SharedRuntimeState {
    fn runtime_config(&self) -> &Arc<RuntimeConfig>;
    fn mail_sender(&self) -> &Arc<dyn MailSender>;
    fn storage_change_tx(&self) -> &tokio::sync::broadcast::Sender<StorageChangeEvent>;
    fn share_download_rollback(&self) -> &ShareDownloadRollbackQueue;
}

pub trait FollowerRuntimeState: SharedRuntimeState {}

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

impl SharedRuntimeState for PrimaryAppState {
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

impl PrimaryRuntimeState for PrimaryAppState {
    fn runtime_config(&self) -> &Arc<RuntimeConfig> {
        &self.runtime_config
    }

    fn mail_sender(&self) -> &Arc<dyn MailSender> {
        &self.mail_sender
    }

    fn storage_change_tx(&self) -> &tokio::sync::broadcast::Sender<StorageChangeEvent> {
        &self.storage_change_tx
    }

    fn share_download_rollback(&self) -> &ShareDownloadRollbackQueue {
        &self.share_download_rollback
    }
}

impl SharedRuntimeState for FollowerAppState {
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

impl FollowerRuntimeState for FollowerAppState {}

impl<T: SharedRuntimeState> SharedRuntimeState for web::Data<T> {
    fn db(&self) -> &DatabaseConnection {
        self.get_ref().db()
    }

    fn driver_registry(&self) -> &Arc<DriverRegistry> {
        self.get_ref().driver_registry()
    }

    fn policy_snapshot(&self) -> &Arc<PolicySnapshot> {
        self.get_ref().policy_snapshot()
    }

    fn config(&self) -> &Arc<Config> {
        self.get_ref().config()
    }

    fn cache(&self) -> &Arc<dyn CacheBackend> {
        self.get_ref().cache()
    }
}

impl<T: PrimaryRuntimeState> PrimaryRuntimeState for web::Data<T> {
    fn runtime_config(&self) -> &Arc<RuntimeConfig> {
        self.get_ref().runtime_config()
    }

    fn mail_sender(&self) -> &Arc<dyn MailSender> {
        self.get_ref().mail_sender()
    }

    fn storage_change_tx(&self) -> &tokio::sync::broadcast::Sender<StorageChangeEvent> {
        self.get_ref().storage_change_tx()
    }

    fn share_download_rollback(&self) -> &ShareDownloadRollbackQueue {
        self.get_ref().share_download_rollback()
    }
}

impl<T: FollowerRuntimeState> FollowerRuntimeState for web::Data<T> {}
