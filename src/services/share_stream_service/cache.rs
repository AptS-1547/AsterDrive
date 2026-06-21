//! 公开分享流式下载计数缓存。

use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use moka::future::Cache;

use crate::cache::CacheExt;
use crate::config::operations;
use crate::runtime::SharedRuntimeState;

use super::CountMarkerState;

const SHARE_STREAM_COUNTED_CACHE_PREFIX: &str = "share_stream_session_counted:";

static FALLBACK_COUNTED_SESSIONS: LazyLock<Cache<String, CountMarkerState>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(StdDuration::from_secs(
            operations::MAX_SHARE_STREAM_SESSION_TTL_SECS,
        ))
        .build()
});

fn counted_cache_key(session_token: &str) -> String {
    format!("{SHARE_STREAM_COUNTED_CACHE_PREFIX}{session_token}")
}

pub(super) async fn load_count_marker(
    state: &impl SharedRuntimeState,
    session_token: &str,
) -> Option<CountMarkerState> {
    let key = counted_cache_key(session_token);
    let primary = state.cache().get::<CountMarkerState>(&key).await;
    let fallback = FALLBACK_COUNTED_SESSIONS.get(&key).await;

    match (primary, fallback) {
        (Some(CountMarkerState::Counted), _) | (_, Some(CountMarkerState::Counted)) => {
            Some(CountMarkerState::Counted)
        }
        (Some(CountMarkerState::Pending), _) | (_, Some(CountMarkerState::Pending)) => {
            Some(CountMarkerState::Pending)
        }
        (None, None) => None,
    }
}

pub(super) async fn reserve_count_marker(
    state: &impl SharedRuntimeState,
    session_token: &str,
    encoded_pending: Vec<u8>,
    ttl_secs: u64,
) -> bool {
    let key = counted_cache_key(session_token);
    if state
        .cache()
        .set_bytes_if_absent(&key, encoded_pending, Some(ttl_secs))
        .await
    {
        FALLBACK_COUNTED_SESSIONS
            .insert(key, CountMarkerState::Pending)
            .await;
        return true;
    }
    false
}

pub(super) async fn store_count_marker(
    state: &impl SharedRuntimeState,
    session_token: &str,
    marker: CountMarkerState,
    encoded_marker: Vec<u8>,
    ttl_secs: u64,
) {
    let key = counted_cache_key(session_token);
    FALLBACK_COUNTED_SESSIONS.insert(key.clone(), marker).await;
    state
        .cache()
        .set_bytes(&key, encoded_marker, Some(ttl_secs))
        .await;
}

pub(super) async fn delete_count_marker(state: &impl SharedRuntimeState, session_token: &str) {
    let key = counted_cache_key(session_token);
    state.cache().delete(&key).await;
    FALLBACK_COUNTED_SESSIONS.remove(&key).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheExt;
    use crate::runtime::test_support::CacheOnlyState;

    fn encoded(marker: CountMarkerState) -> Vec<u8> {
        serde_json::to_vec(&marker).expect("marker should serialize")
    }

    #[tokio::test]
    async fn count_marker_reservation_is_single_use_and_delete_clears_it() {
        let state = CacheOnlyState::new().await;

        assert!(
            reserve_count_marker(&state, "session-a", encoded(CountMarkerState::Pending), 60).await
        );
        assert!(
            !reserve_count_marker(&state, "session-a", encoded(CountMarkerState::Pending), 60)
                .await
        );
        assert_eq!(
            load_count_marker(&state, "session-a").await,
            Some(CountMarkerState::Pending)
        );

        delete_count_marker(&state, "session-a").await;

        assert!(load_count_marker(&state, "session-a").await.is_none());
    }

    #[tokio::test]
    async fn counted_marker_has_priority_over_pending_marker() {
        let state = CacheOnlyState::new().await;
        let key = counted_cache_key("session-priority");

        state
            .cache()
            .set(&key, &CountMarkerState::Pending, Some(60))
            .await;
        FALLBACK_COUNTED_SESSIONS
            .insert(key, CountMarkerState::Counted)
            .await;

        assert_eq!(
            load_count_marker(&state, "session-priority").await,
            Some(CountMarkerState::Counted)
        );
    }

    #[tokio::test]
    async fn storing_counted_updates_primary_and_fallback() {
        let state = CacheOnlyState::new().await;

        store_count_marker(
            &state,
            "session-counted",
            CountMarkerState::Counted,
            encoded(CountMarkerState::Counted),
            60,
        )
        .await;

        assert_eq!(
            load_count_marker(&state, "session-counted").await,
            Some(CountMarkerState::Counted)
        );
    }
}
