//! 缓存实现：`memory`。

use super::CacheBackend;
use async_trait::async_trait;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

const MEMORY_CACHE_MAX_BYTES: u64 = 64 * 1024 * 1024;

pub struct MemoryCache {
    cache: Cache<String, Vec<u8>>,
}

impl MemoryCache {
    pub fn new(default_ttl: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(MEMORY_CACHE_MAX_BYTES)
            .weigher(|key: &String, value: &Vec<u8>| entry_weight(key.len(), value.len()))
            .time_to_live(Duration::from_secs(default_ttl))
            .build();
        Self { cache }
    }
}

fn entry_weight(key_len: usize, value_len: usize) -> u32 {
    let total = key_len.saturating_add(value_len);
    u32::try_from(total).unwrap_or(u32::MAX)
}

#[async_trait]
impl CacheBackend for MemoryCache {
    async fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        self.cache.get(key).await
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>, _ttl_secs: Option<u64>) {
        // moka 用全局 TTL，per-entry TTL 需要 Expiry trait（后续可加）
        self.cache.insert(key.to_string(), value).await;
    }

    async fn delete(&self, key: &str) {
        self.cache.remove(key).await;
    }

    async fn invalidate_prefix(&self, prefix: &str) {
        let keys: Vec<Arc<String>> = self
            .cache
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            self.cache.remove(key.as_ref()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::entry_weight;

    #[test]
    fn entry_weight_counts_key_and_value_bytes() {
        assert_eq!(entry_weight(3, 5), 8);
    }

    #[test]
    fn entry_weight_saturates_at_u32_max() {
        assert_eq!(entry_weight(usize::MAX, usize::MAX), u32::MAX);
    }
}
