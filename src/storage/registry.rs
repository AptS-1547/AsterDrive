use super::driver::StorageDriver;
use super::local::LocalDriver;
use crate::entities::storage_policy;
use crate::errors::Result;
use dashmap::DashMap;
use std::sync::Arc;

pub struct DriverRegistry {
    /// policy_id → 已实例化的 driver
    drivers: DashMap<i64, Arc<dyn StorageDriver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: DashMap::new(),
        }
    }

    /// 根据 StoragePolicy 获取或创建 driver（惰性实例化）
    pub fn get_driver(&self, policy: &storage_policy::Model) -> Result<Arc<dyn StorageDriver>> {
        if let Some(driver) = self.drivers.get(&policy.id) {
            return Ok(driver.clone());
        }
        let driver = self.create_driver(policy)?;
        self.drivers.insert(policy.id, driver.clone());
        Ok(driver)
    }

    /// 策略更新后使缓存的 driver 失效
    pub fn invalidate(&self, policy_id: i64) {
        self.drivers.remove(&policy_id);
    }

    #[cfg(test)]
    pub fn insert_for_test(&self, policy_id: i64, driver: Arc<dyn StorageDriver>) {
        self.drivers.insert(policy_id, driver);
    }

    fn create_driver(&self, policy: &storage_policy::Model) -> Result<Arc<dyn StorageDriver>> {
        use super::s3::S3Driver;
        use crate::types::DriverType;
        match policy.driver_type {
            DriverType::Local => Ok(Arc::new(LocalDriver::new(policy)?)),
            DriverType::S3 => Ok(Arc::new(S3Driver::new(policy)?)),
        }
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}
