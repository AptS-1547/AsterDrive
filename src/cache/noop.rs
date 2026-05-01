//! 缓存实现：`noop`。

use super::CacheBackend;
use async_trait::async_trait;
use moka::future::Cache;
use std::time::Duration;

const NOOP_RESERVATION_MAX_ENTRIES: u64 = 64 * 1024;

pub struct NoopCache {
    reservations: Cache<String, Vec<u8>>,
}

impl NoopCache {
    pub fn new(default_ttl: u64) -> Self {
        let reservations = Cache::builder()
            .max_capacity(NOOP_RESERVATION_MAX_ENTRIES)
            .time_to_live(Duration::from_secs(default_ttl))
            .build();
        Self { reservations }
    }
}

#[async_trait]
impl CacheBackend for NoopCache {
    async fn get_bytes(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }

    async fn set_bytes(&self, _key: &str, _value: Vec<u8>, _ttl_secs: Option<u64>) {}

    async fn set_bytes_if_absent(&self, key: &str, value: Vec<u8>, _ttl_secs: Option<u64>) -> bool {
        self.reservations
            .entry(key.to_string())
            .or_insert(value)
            .await
            .is_fresh()
    }

    async fn delete(&self, _key: &str) {}

    async fn invalidate_prefix(&self, _prefix: &str) {}
}
