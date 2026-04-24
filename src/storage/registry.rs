//! 存储子模块：`registry`。

use super::StorageErrorKind;
use super::driver::StorageDriver;
use super::drivers::local::LocalDriver;
use super::drivers::remote::RemoteDriver;
use super::drivers::s3::S3Driver;
use super::error::storage_driver_error;
use super::multipart::MultipartStorageDriver;
use crate::db::repository::{managed_follower_repo, master_binding_repo};
use crate::entities::storage_policy;
use crate::errors::{Result, precondition_failed_with_subcode};
use crate::types::DriverType;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 已实例化的 driver，按类型区分以支持 multipart downcast。
#[derive(Clone)]
enum DriverEntry {
    Local(Arc<LocalDriver>),
    Remote(Arc<RemoteDriver>),
    S3(Arc<S3Driver>),
    #[cfg(test)]
    Mock(Arc<dyn StorageDriver>),
}

impl DriverEntry {
    fn as_storage_driver(&self) -> Arc<dyn StorageDriver> {
        match self {
            DriverEntry::Local(d) => d.clone(),
            DriverEntry::Remote(d) => d.clone(),
            DriverEntry::S3(d) => d.clone(),
            #[cfg(test)]
            DriverEntry::Mock(d) => d.clone(),
        }
    }

    fn as_multipart_driver(&self) -> Option<Arc<dyn MultipartStorageDriver>> {
        match self {
            DriverEntry::Local(_) => None,
            DriverEntry::Remote(d) => Some(d.clone()),
            DriverEntry::S3(d) => Some(d.clone()),
            #[cfg(test)]
            DriverEntry::Mock(_) => None,
        }
    }
}

pub struct DriverRegistry {
    /// policy_id → 已实例化的 driver
    drivers: DashMap<i64, DriverEntry>,
    managed_followers_by_id: RwLock<HashMap<i64, crate::entities::managed_follower::Model>>,
    master_bindings_by_access_key: RwLock<HashMap<String, crate::entities::master_binding::Model>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: DashMap::new(),
            managed_followers_by_id: RwLock::new(HashMap::new()),
            master_bindings_by_access_key: RwLock::new(HashMap::new()),
        }
    }

    /// 根据 StoragePolicy 获取或创建 driver（惰性实例化）
    pub fn get_driver(&self, policy: &storage_policy::Model) -> Result<Arc<dyn StorageDriver>> {
        Ok(self.get_entry(policy)?.as_storage_driver())
    }

    /// 获取支持 multipart upload 的 driver。
    ///
    /// 如果策略对应的 driver 不支持 multipart（如 LocalDriver），返回 `Err`。
    pub fn get_multipart_driver(
        &self,
        policy: &storage_policy::Model,
    ) -> Result<Arc<dyn MultipartStorageDriver>> {
        self.get_entry(policy)?
            .as_multipart_driver()
            .ok_or_else(|| {
                storage_driver_error(
                    StorageErrorKind::Unsupported,
                    format!(
                        "storage policy {} (driver: {:?}) does not support multipart upload",
                        policy.id, policy.driver_type
                    ),
                )
            })
    }

    /// 策略更新后使缓存的 driver 失效
    pub fn invalidate(&self, policy_id: i64) {
        self.drivers.remove(&policy_id);
    }

    pub fn invalidate_all(&self) {
        self.drivers.clear();
    }

    pub async fn reload_primary_state<C: sea_orm::ConnectionTrait>(&self, db: &C) -> Result<()> {
        self.reload_managed_followers(db).await?;
        self.reload_master_bindings(db).await
    }

    pub async fn reload_follower_state<C: sea_orm::ConnectionTrait>(&self, db: &C) -> Result<()> {
        self.reload_master_bindings(db).await
    }

    pub async fn reload_managed_followers<C: sea_orm::ConnectionTrait>(
        &self,
        db: &C,
    ) -> Result<()> {
        let followers = managed_follower_repo::find_all(db).await?;
        let mut by_id = HashMap::with_capacity(followers.len());
        for follower in followers {
            by_id.insert(follower.id, follower);
        }
        *self.managed_followers_by_id.write() = by_id;
        Ok(())
    }

    pub async fn reload_master_bindings<C: sea_orm::ConnectionTrait>(&self, db: &C) -> Result<()> {
        let bindings = master_binding_repo::find_all(db).await?;
        let mut by_access_key = HashMap::with_capacity(bindings.len());
        for binding in bindings {
            by_access_key.insert(binding.access_key.clone(), binding);
        }
        *self.master_bindings_by_access_key.write() = by_access_key;
        Ok(())
    }

    pub fn get_managed_follower(
        &self,
        follower_id: i64,
    ) -> Option<crate::entities::managed_follower::Model> {
        self.managed_followers_by_id
            .read()
            .get(&follower_id)
            .cloned()
    }

    pub fn find_master_binding_by_access_key(
        &self,
        access_key: &str,
    ) -> Option<crate::entities::master_binding::Model> {
        self.master_bindings_by_access_key
            .read()
            .get(access_key)
            .cloned()
    }

    #[cfg(test)]
    pub fn insert_for_test(&self, policy_id: i64, driver: Arc<dyn StorageDriver>) {
        self.drivers.insert(policy_id, DriverEntry::Mock(driver));
    }

    #[cfg(test)]
    pub fn insert_s3_for_test(&self, policy_id: i64, driver: Arc<S3Driver>) {
        self.drivers.insert(policy_id, DriverEntry::S3(driver));
    }

    fn get_entry(&self, policy: &storage_policy::Model) -> Result<DriverEntry> {
        if let Some(entry) = self.drivers.get(&policy.id) {
            return Ok(entry.clone());
        }
        let entry = self.create_entry(policy)?;
        self.drivers.insert(policy.id, entry.clone());
        Ok(entry)
    }

    fn create_entry(&self, policy: &storage_policy::Model) -> Result<DriverEntry> {
        match policy.driver_type {
            DriverType::Local => Ok(DriverEntry::Local(Arc::new(LocalDriver::new(policy)?))),
            DriverType::Remote => {
                let remote_node_id = policy.remote_node_id.ok_or_else(|| {
                    storage_driver_error(
                        StorageErrorKind::Misconfigured,
                        "remote storage policy missing remote_node_id",
                    )
                })?;
                let remote_node = self.get_managed_follower(remote_node_id).ok_or_else(|| {
                    storage_driver_error(
                        StorageErrorKind::Misconfigured,
                        format!("remote node #{remote_node_id} not loaded in registry"),
                    )
                })?;
                if !remote_node.is_enabled {
                    return Err(precondition_failed_with_subcode(
                        "remote_node.disabled",
                        format!("remote node #{remote_node_id} is disabled"),
                    ));
                }
                Ok(DriverEntry::Remote(Arc::new(RemoteDriver::new(
                    policy,
                    &remote_node,
                )?)))
            }
            DriverType::S3 => Ok(DriverEntry::S3(Arc::new(S3Driver::new(policy)?))),
        }
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}
