use super::driver::StorageDriver;
use super::drivers::local::LocalDriver;
use super::drivers::s3::S3Driver;
use super::multipart::MultipartStorageDriver;
use crate::entities::storage_policy;
use crate::errors::{AsterError, Result};
use crate::types::DriverType;
use dashmap::DashMap;
use std::sync::Arc;

/// 已实例化的 driver，按类型区分以支持 multipart downcast。
#[derive(Clone)]
enum DriverEntry {
    Local(Arc<LocalDriver>),
    S3(Arc<S3Driver>),
    #[cfg(test)]
    Mock(Arc<dyn StorageDriver>),
}

impl DriverEntry {
    fn as_storage_driver(&self) -> Arc<dyn StorageDriver> {
        match self {
            DriverEntry::Local(d) => d.clone(),
            DriverEntry::S3(d) => d.clone(),
            #[cfg(test)]
            DriverEntry::Mock(d) => d.clone(),
        }
    }

    fn as_multipart_driver(&self) -> Option<Arc<dyn MultipartStorageDriver>> {
        match self {
            DriverEntry::Local(_) => None,
            DriverEntry::S3(d) => Some(d.clone()),
            #[cfg(test)]
            DriverEntry::Mock(_) => None,
        }
    }
}

pub struct DriverRegistry {
    /// policy_id → 已实例化的 driver
    drivers: DashMap<i64, DriverEntry>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: DashMap::new(),
        }
    }

    /// 根据 StoragePolicy 获取或创建 driver（惰性实例化）
    pub fn get_driver(&self, policy: &storage_policy::Model) -> Result<Arc<dyn StorageDriver>> {
        Ok(self.get_entry(policy)?.as_storage_driver())
    }

    /// 获取支持 multipart upload 的 driver（仅 S3 类策略）。
    ///
    /// 如果策略对应的 driver 不支持 multipart（如 LocalDriver），返回 `Err`。
    pub fn get_multipart_driver(
        &self,
        policy: &storage_policy::Model,
    ) -> Result<Arc<dyn MultipartStorageDriver>> {
        self.get_entry(policy)?
            .as_multipart_driver()
            .ok_or_else(|| {
                AsterError::storage_driver_error(format!(
                    "storage policy {} (driver: {:?}) does not support multipart upload",
                    policy.id, policy.driver_type
                ))
            })
    }

    /// 策略更新后使缓存的 driver 失效
    pub fn invalidate(&self, policy_id: i64) {
        self.drivers.remove(&policy_id);
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
            DriverType::S3 => Ok(DriverEntry::S3(Arc::new(S3Driver::new(policy)?))),
        }
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}
