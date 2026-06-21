//! 预览链接使用次数缓存。

use crate::runtime::SharedRuntimeState;

const PREVIEW_LINK_CACHE_PREFIX: &str = "preview_link:";

fn usage_slot_key(token: &str, slot: u32) -> String {
    format!("{PREVIEW_LINK_CACHE_PREFIX}{token}:use:{slot}")
}

pub(super) async fn reserve_usage_slot(
    state: &impl SharedRuntimeState,
    token: &str,
    slot: u32,
    marker: Vec<u8>,
    ttl_secs: u64,
) -> bool {
    state
        .cache()
        .set_bytes_if_absent(&usage_slot_key(token, slot), marker, Some(ttl_secs))
        .await
}

pub(super) async fn release_usage_slot(state: &impl SharedRuntimeState, token: &str, slot: u32) {
    state.cache().delete(&usage_slot_key(token, slot)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::CacheOnlyState;

    #[tokio::test]
    async fn usage_slot_reservation_is_single_use_per_slot() {
        let state = CacheOnlyState::new().await;

        assert!(reserve_usage_slot(&state, "token-a", 0, Vec::new(), 60).await);
        assert!(!reserve_usage_slot(&state, "token-a", 0, Vec::new(), 60).await);
        assert!(reserve_usage_slot(&state, "token-a", 1, Vec::new(), 60).await);
        assert!(reserve_usage_slot(&state, "token-b", 0, Vec::new(), 60).await);
    }

    #[tokio::test]
    async fn usage_slot_release_allows_reservation_again() {
        let state = CacheOnlyState::new().await;

        assert!(reserve_usage_slot(&state, "token-a", 0, Vec::new(), 60).await);
        release_usage_slot(&state, "token-a", 0).await;

        assert!(reserve_usage_slot(&state, "token-a", 0, Vec::new(), 60).await);
    }

    #[tokio::test]
    async fn usage_slot_zero_ttl_expires_immediately() {
        let state = CacheOnlyState::new().await;

        assert!(reserve_usage_slot(&state, "token-a", 0, Vec::new(), 0).await);

        assert!(reserve_usage_slot(&state, "token-a", 0, Vec::new(), 60).await);
    }
}
