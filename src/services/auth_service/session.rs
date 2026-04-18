//! 认证服务子模块：`session`。

use chrono::Utc;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

use crate::cache::CacheExt;
use crate::db::repository::user_repo;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;

use super::{AUTH_SNAPSHOT_TTL, AuthSnapshot, UserAuditInfo, user_audit_info};

fn auth_snapshot_cache_key(user_id: i64) -> String {
    format!("auth_snapshot:{user_id}")
}

pub async fn get_auth_snapshot(state: &AppState, user_id: i64) -> Result<AuthSnapshot> {
    let cache_key = auth_snapshot_cache_key(user_id);
    if let Some(snapshot) = state.cache.get(&cache_key).await {
        tracing::debug!(user_id, "auth snapshot cache hit");
        return Ok(snapshot);
    }

    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let snapshot = AuthSnapshot::from_user(&user);
    state
        .cache
        .set(&cache_key, &snapshot, Some(AUTH_SNAPSHOT_TTL))
        .await;
    tracing::debug!(user_id, "auth snapshot cache miss");
    Ok(snapshot)
}

pub async fn invalidate_auth_snapshot_cache(state: &AppState, user_id: i64) {
    state.cache.delete(&auth_snapshot_cache_key(user_id)).await;
}

pub async fn revoke_user_sessions(state: &AppState, user_id: i64) -> Result<UserAuditInfo> {
    tracing::debug!(user_id, "revoking user sessions");
    let user = user_repo::find_by_id(&state.db, user_id).await?;
    let next_session_version = user.session_version.saturating_add(1);
    let mut active = user.into_active_model();
    active.session_version = Set(next_session_version);
    active.updated_at = Set(Utc::now());
    let updated = active
        .update(&state.db)
        .await
        .map_aster_err(AsterError::database_operation)?;
    invalidate_auth_snapshot_cache(state, updated.id).await;
    tracing::debug!(
        user_id = updated.id,
        session_version = updated.session_version,
        "revoked user sessions"
    );
    Ok(user_audit_info(&updated))
}
