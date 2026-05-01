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

    async fn set_bytes_if_absent(&self, key: &str, value: Vec<u8>, _ttl_secs: Option<u64>) -> bool {
        // moka entry API 对同一个 key 的并发插入会合并，is_fresh 只会有一个 true。
        self.cache
            .entry(key.to_string())
            .or_insert(value)
            .await
            .is_fresh()
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
    use super::{CacheBackend, MemoryCache, entry_weight};
    use std::sync::Arc;

    #[test]
    fn entry_weight_counts_key_and_value_bytes() {
        assert_eq!(entry_weight(3, 5), 8);
    }

    #[test]
    fn entry_weight_saturates_at_u32_max() {
        assert_eq!(entry_weight(usize::MAX, usize::MAX), u32::MAX);
    }

    #[tokio::test]
    async fn set_bytes_if_absent_allows_one_concurrent_insert() {
        let cache = Arc::new(MemoryCache::new(60));
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let cache = cache.clone();
            tasks.push(tokio::spawn(async move {
                cache
                    .set_bytes_if_absent("nonce", Vec::new(), Some(60))
                    .await
            }));
        }

        let successes = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|result| result.expect("reservation task should not panic"))
            .filter(|inserted| *inserted)
            .count();

        assert_eq!(successes, 1);
    }
}
