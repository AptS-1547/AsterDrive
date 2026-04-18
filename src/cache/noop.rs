//! 缓存实现：`noop`。

use super::CacheBackend;
use async_trait::async_trait;

pub struct NoopCache;

#[async_trait]
impl CacheBackend for NoopCache {
    async fn get_bytes(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }

    async fn set_bytes(&self, _key: &str, _value: Vec<u8>, _ttl_secs: Option<u64>) {}

    async fn delete(&self, _key: &str) {}

    async fn invalidate_prefix(&self, _prefix: &str) {}
}
