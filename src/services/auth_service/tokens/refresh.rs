use chrono::Utc;
use dashmap::DashMap;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::db::repository::{auth_session_repo, user_repo};
use crate::entities::auth_session;
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::audit_service::{self, AuditContext};
use crate::types::TokenType;

use super::super::Claims;
use super::super::session::{
    invalidate_auth_snapshot_cache, purge_all_auth_sessions_in_connection,
};
use super::{ensure_token_type, issue_tokens_for_session_id, verify_token};

const REFRESH_REUSE_GRACE_SECS: i64 = 15;

static REFRESH_ROTATION_LOCKS: LazyLock<DashMap<String, Arc<Mutex<()>>>> =
    LazyLock::new(DashMap::new);

#[derive(Debug)]
enum RefreshRotationError {
    Aster(AsterError),
    StaleRefresh {
        user_id: i64,
        reused_jti: String,
    },
    ReuseDetected {
        user_id: i64,
        reused_jti: String,
    },
    RotateConflict {
        ip_address: Option<String>,
        user_agent: Option<String>,
    },
}

#[derive(Serialize)]
struct RefreshTokenReuseAuditDetails<'a> {
    reused_jti: &'a str,
}

struct RefreshRotationLockGuard {
    refresh_jti: String,
    lock: Arc<Mutex<()>>,
    _guard: OwnedMutexGuard<()>,
}

impl Drop for RefreshRotationLockGuard {
    fn drop(&mut self) {
        REFRESH_ROTATION_LOCKS.remove_if(&self.refresh_jti, |_, lock| {
            Arc::ptr_eq(lock, &self.lock) && Arc::strong_count(lock) == 3
        });
    }
}

impl From<AsterError> for RefreshRotationError {
    fn from(value: AsterError) -> Self {
        Self::Aster(value)
    }
}

fn is_recent_refresh_rotation(session: &auth_session::Model, now: chrono::DateTime<Utc>) -> bool {
    now.signed_duration_since(session.last_seen_at)
        .num_seconds()
        .abs()
        <= REFRESH_REUSE_GRACE_SECS
}

fn refresh_client_matches(
    session: &auth_session::Model,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> bool {
    let Some(stored_user_agent) = session.user_agent.as_deref() else {
        return false;
    };
    let Some(current_user_agent) = user_agent else {
        return false;
    };
    if stored_user_agent != current_user_agent {
        return false;
    }

    session.ip_address.is_none()
        || ip_address.is_none()
        || session.ip_address.as_deref() == ip_address
}

fn is_stale_refresh_from_same_client(
    session: &auth_session::Model,
    user_id: i64,
    now: chrono::DateTime<Utc>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> bool {
    session.user_id == user_id
        && session.revoked_at.is_none()
        && is_recent_refresh_rotation(session, now)
        && refresh_client_matches(session, ip_address, user_agent)
}

fn classify_refresh_reuse_session(
    reused_auth_session: Option<&auth_session::Model>,
    user_id: i64,
    refresh_jti: &str,
    now: chrono::DateTime<Utc>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> RefreshRotationError {
    if reused_auth_session.is_some_and(|session| {
        is_stale_refresh_from_same_client(session, user_id, now, ip_address, user_agent)
    }) {
        return RefreshRotationError::StaleRefresh {
            user_id,
            reused_jti: refresh_jti.to_string(),
        };
    }

    RefreshRotationError::ReuseDetected {
        user_id,
        reused_jti: refresh_jti.to_string(),
    }
}

fn classify_refresh_rotation_lock_loss_session(
    reused_auth_session: Option<&auth_session::Model>,
    user_id: i64,
    refresh_jti: &str,
    now: chrono::DateTime<Utc>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> RefreshRotationError {
    if reused_auth_session.is_some_and(|session| {
        session.user_id == user_id
            && session.revoked_at.is_none()
            && is_recent_refresh_rotation(session, now)
    }) {
        return RefreshRotationError::StaleRefresh {
            user_id,
            reused_jti: refresh_jti.to_string(),
        };
    }

    classify_refresh_reuse_session(
        reused_auth_session,
        user_id,
        refresh_jti,
        now,
        ip_address,
        user_agent,
    )
}

fn try_acquire_refresh_rotation_lock(refresh_jti: &str) -> Option<RefreshRotationLockGuard> {
    let lock = REFRESH_ROTATION_LOCKS
        .entry(refresh_jti.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    lock.clone()
        .try_lock_owned()
        .ok()
        .map(|guard| RefreshRotationLockGuard {
            refresh_jti: refresh_jti.to_string(),
            lock,
            _guard: guard,
        })
}

async fn wait_for_refresh_rotation_lock(refresh_jti: &str) -> RefreshRotationLockGuard {
    let lock = REFRESH_ROTATION_LOCKS
        .entry(refresh_jti.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let guard = lock.clone().lock_owned().await;
    RefreshRotationLockGuard {
        refresh_jti: refresh_jti.to_string(),
        lock,
        _guard: guard,
    }
}

async fn classify_refresh_rotation_lock_loss<C: ConnectionTrait>(
    db: &C,
    claims: &Claims,
    refresh_jti: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<RefreshRotationError> {
    let now = Utc::now();
    let reused_auth_session =
        auth_session_repo::find_by_previous_refresh_jti(db, refresh_jti).await?;
    Ok(classify_refresh_rotation_lock_loss_session(
        reused_auth_session.as_ref(),
        claims.user_id,
        refresh_jti,
        now,
        ip_address,
        user_agent,
    ))
}

async fn classify_failed_refresh_rotation<C: ConnectionTrait>(
    db: &C,
    claims: &Claims,
    refresh_jti: &str,
    now: chrono::DateTime<Utc>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<RefreshRotationError> {
    let reused_auth_session =
        auth_session_repo::find_by_previous_refresh_jti(db, refresh_jti).await?;
    Ok(classify_refresh_reuse_session(
        reused_auth_session.as_ref(),
        claims.user_id,
        refresh_jti,
        now,
        ip_address,
        user_agent,
    ))
}

pub async fn refresh_tokens(
    state: &PrimaryAppState,
    refresh: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(String, String)> {
    tracing::debug!("refreshing auth tokens");
    let claims = verify_token(refresh, &state.config.auth.jwt_secret)?;
    ensure_token_type(&claims, TokenType::Refresh)?;
    let refresh_jti = claims
        .jti
        .clone()
        .ok_or_else(|| AsterError::auth_token_invalid("refresh token missing jti"))?;
    let Some(_rotation_lock_guard) = try_acquire_refresh_rotation_lock(&refresh_jti) else {
        tracing::debug!(
            user_id = claims.user_id,
            reused_jti = refresh_jti,
            "concurrent refresh token rotation lost same-token race"
        );
        let _winner_lock_guard = wait_for_refresh_rotation_lock(&refresh_jti).await;
        let txn = crate::db::transaction::begin(&state.db).await?;
        let lock_loss = classify_refresh_rotation_lock_loss(
            &txn,
            &claims,
            &refresh_jti,
            ip_address,
            user_agent,
        )
        .await?;
        return match lock_loss {
            RefreshRotationError::StaleRefresh {
                user_id,
                reused_jti,
            } => {
                crate::db::transaction::rollback(txn).await?;
                tracing::debug!(
                    user_id,
                    reused_jti,
                    "stale refresh token reused within rotation grace window"
                );
                Err(AsterError::auth_token_invalid("stale refresh token"))
            }
            RefreshRotationError::ReuseDetected {
                user_id,
                reused_jti,
            } => {
                user_repo::bump_session_version(&txn, user_id).await?;
                purge_all_auth_sessions_in_connection(&txn, user_id).await?;
                crate::db::transaction::commit(txn).await?;
                invalidate_auth_snapshot_cache(state, user_id).await;
                tracing::warn!(
                    user_id,
                    reused_jti,
                    "refresh token reuse detected after losing rotation lock; revoked all sessions"
                );
                audit_service::log(
                    state,
                    &AuditContext {
                        user_id,
                        ip_address: None,
                        user_agent: None,
                    },
                    audit_service::AuditAction::UserRefreshTokenReuseDetected,
                    crate::services::audit_service::AuditEntityType::User,
                    Some(user_id),
                    None,
                    audit_service::details(RefreshTokenReuseAuditDetails {
                        reused_jti: &reused_jti,
                    }),
                )
                .await;
                Err(AsterError::auth_token_invalid(
                    "refresh token reuse detected",
                ))
            }
            RefreshRotationError::Aster(error) => {
                crate::db::transaction::rollback(txn).await?;
                Err(error)
            }
            RefreshRotationError::RotateConflict { .. } => {
                crate::db::transaction::rollback(txn).await?;
                Err(AsterError::auth_token_invalid("stale refresh token"))
            }
        };
    };

    let txn = crate::db::transaction::begin(&state.db).await?;
    let rotation = async {
        let now = Utc::now();
        let existing_auth_session = auth_session_repo::find_by_refresh_jti(&txn, &refresh_jti)
            .await
            .map_err(RefreshRotationError::from)?;
        let reused_auth_session = if existing_auth_session.is_none() {
            auth_session_repo::find_by_previous_refresh_jti(&txn, &refresh_jti)
                .await
                .map_err(RefreshRotationError::from)?
        } else {
            None
        };
        let user = user_repo::find_by_id(&txn, claims.user_id)
            .await
            .map_err(RefreshRotationError::from)?;
        if !user.status.is_active() {
            return Err(RefreshRotationError::from(AsterError::auth_forbidden(
                "account is disabled",
            )));
        }
        if claims.session_version != user.session_version {
            return Err(RefreshRotationError::from(AsterError::auth_token_invalid(
                "session revoked",
            )));
        }

        let Some(existing_auth_session) = existing_auth_session else {
            if reused_auth_session.as_ref().is_some_and(|session| {
                session.user_id == claims.user_id && session.revoked_at.is_none()
            }) {
                let stale_refresh_from_same_client =
                    reused_auth_session.as_ref().is_some_and(|session| {
                        is_stale_refresh_from_same_client(
                            session,
                            claims.user_id,
                            now,
                            ip_address,
                            user_agent,
                        )
                    });
                if stale_refresh_from_same_client {
                    return Err(RefreshRotationError::StaleRefresh {
                        user_id: claims.user_id,
                        reused_jti: refresh_jti.clone(),
                    });
                }
                user_repo::bump_session_version(&txn, claims.user_id)
                    .await
                    .map_err(RefreshRotationError::from)?;
                purge_all_auth_sessions_in_connection(&txn, claims.user_id)
                    .await
                    .map_err(RefreshRotationError::from)?;
                return Err(RefreshRotationError::ReuseDetected {
                    user_id: claims.user_id,
                    reused_jti: refresh_jti.clone(),
                });
            }
            return Err(RefreshRotationError::from(AsterError::auth_token_invalid(
                "session revoked",
            )));
        };

        if existing_auth_session.user_id != claims.user_id {
            return Err(RefreshRotationError::from(AsterError::auth_token_invalid(
                "invalid token",
            )));
        }
        if existing_auth_session.revoked_at.is_some() {
            return Err(RefreshRotationError::from(AsterError::auth_token_invalid(
                "session revoked",
            )));
        }

        let next_ip_address = ip_address.or(existing_auth_session.ip_address.as_deref());
        let next_user_agent = user_agent.or(existing_auth_session.user_agent.as_deref());
        let tokens = issue_tokens_for_session_id(
            state,
            user.id,
            user.session_version,
            Some(existing_auth_session.id.as_str()),
        )
        .map_err(RefreshRotationError::from)?;

        if !auth_session_repo::rotate_refresh(
            &txn,
            &refresh_jti,
            &tokens.refresh_jti,
            tokens.refresh_expires_at,
            next_ip_address,
            next_user_agent,
            now,
        )
        .await
        .map_err(RefreshRotationError::from)?
        {
            return Err(RefreshRotationError::RotateConflict {
                ip_address: next_ip_address.map(str::to_string),
                user_agent: next_user_agent.map(str::to_string),
            });
        }

        Ok::<_, RefreshRotationError>((
            (tokens.access_token, tokens.refresh_token),
            user.session_version,
        ))
    }
    .await;

    match rotation {
        Ok((tokens, session_version)) => {
            crate::db::transaction::commit(txn).await?;
            tracing::debug!(
                user_id = claims.user_id,
                session_version,
                "refreshed auth tokens"
            );
            Ok(tokens)
        }
        Err(RefreshRotationError::StaleRefresh {
            user_id,
            reused_jti,
        }) => {
            crate::db::transaction::rollback(txn).await?;
            tracing::debug!(
                user_id,
                reused_jti,
                "stale refresh token reused within rotation grace window"
            );
            Err(AsterError::auth_token_invalid("stale refresh token"))
        }
        Err(RefreshRotationError::ReuseDetected {
            user_id,
            reused_jti,
        }) => {
            crate::db::transaction::commit(txn).await?;
            invalidate_auth_snapshot_cache(state, user_id).await;
            tracing::warn!(
                user_id,
                reused_jti,
                "refresh token reuse detected; revoked all sessions"
            );
            audit_service::log(
                state,
                &AuditContext {
                    user_id,
                    ip_address: None,
                    user_agent: None,
                },
                audit_service::AuditAction::UserRefreshTokenReuseDetected,
                crate::services::audit_service::AuditEntityType::User,
                Some(user_id),
                None,
                audit_service::details(RefreshTokenReuseAuditDetails {
                    reused_jti: &reused_jti,
                }),
            )
            .await;
            Err(AsterError::auth_token_invalid(
                "refresh token reuse detected",
            ))
        }
        Err(RefreshRotationError::RotateConflict {
            ip_address,
            user_agent,
        }) => {
            crate::db::transaction::rollback(txn).await?;
            let txn = crate::db::transaction::begin(&state.db).await?;
            let rotation_loss = classify_failed_refresh_rotation(
                &txn,
                &claims,
                &refresh_jti,
                Utc::now(),
                ip_address.as_deref(),
                user_agent.as_deref(),
            )
            .await?;
            match rotation_loss {
                RefreshRotationError::StaleRefresh {
                    user_id,
                    reused_jti,
                } => {
                    crate::db::transaction::rollback(txn).await?;
                    tracing::debug!(
                        user_id,
                        reused_jti,
                        "stale refresh token reused within rotation grace window"
                    );
                    Err(AsterError::auth_token_invalid("stale refresh token"))
                }
                RefreshRotationError::ReuseDetected {
                    user_id,
                    reused_jti,
                } => {
                    user_repo::bump_session_version(&txn, user_id).await?;
                    purge_all_auth_sessions_in_connection(&txn, user_id).await?;
                    crate::db::transaction::commit(txn).await?;
                    invalidate_auth_snapshot_cache(state, user_id).await;
                    tracing::warn!(
                        user_id,
                        reused_jti,
                        "refresh token reuse detected after refresh rotation conflict; revoked all sessions"
                    );
                    audit_service::log(
                        state,
                        &AuditContext {
                            user_id,
                            ip_address: None,
                            user_agent: None,
                        },
                        audit_service::AuditAction::UserRefreshTokenReuseDetected,
                        crate::services::audit_service::AuditEntityType::User,
                        Some(user_id),
                        None,
                        audit_service::details(RefreshTokenReuseAuditDetails {
                            reused_jti: &reused_jti,
                        }),
                    )
                    .await;
                    Err(AsterError::auth_token_invalid(
                        "refresh token reuse detected",
                    ))
                }
                RefreshRotationError::Aster(error) => {
                    crate::db::transaction::rollback(txn).await?;
                    Err(error)
                }
                RefreshRotationError::RotateConflict { .. } => {
                    crate::db::transaction::rollback(txn).await?;
                    Err(AsterError::auth_token_invalid("stale refresh token"))
                }
            }
        }
        Err(RefreshRotationError::Aster(error)) => {
            crate::db::transaction::rollback(txn).await?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn make_auth_session(now: chrono::DateTime<Utc>) -> auth_session::Model {
        auth_session::Model {
            id: "session-id".to_string(),
            user_id: 1,
            current_refresh_jti: "next-jti".to_string(),
            previous_refresh_jti: Some("refresh-jti".to_string()),
            refresh_expires_at: now + ChronoDuration::days(1),
            ip_address: Some("203.0.113.10".to_string()),
            user_agent: Some("browser".to_string()),
            created_at: now,
            last_seen_at: now,
            revoked_at: None,
        }
    }

    fn assert_stale_refresh(error: RefreshRotationError, user_id: i64, reused_jti: &str) {
        match error {
            RefreshRotationError::StaleRefresh {
                user_id: actual_user_id,
                reused_jti: actual_reused_jti,
            } => {
                assert_eq!(actual_user_id, user_id);
                assert_eq!(actual_reused_jti, reused_jti);
            }
            other => panic!("expected stale refresh, got {other:?}"),
        }
    }

    fn assert_reuse_detected(error: RefreshRotationError, user_id: i64, reused_jti: &str) {
        match error {
            RefreshRotationError::ReuseDetected {
                user_id: actual_user_id,
                reused_jti: actual_reused_jti,
            } => {
                assert_eq!(actual_user_id, user_id);
                assert_eq!(actual_reused_jti, reused_jti);
            }
            other => panic!("expected refresh reuse, got {other:?}"),
        }
    }

    #[test]
    fn classify_refresh_reuse_session_treats_same_client_grace_as_stale() {
        let now = Utc::now();
        let session = make_auth_session(now);

        let result = classify_refresh_reuse_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            Some("203.0.113.10"),
            Some("browser"),
        );

        assert_stale_refresh(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_reuse_session_treats_different_client_as_reuse() {
        let now = Utc::now();
        let session = make_auth_session(now);

        let result = classify_refresh_reuse_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            Some("203.0.113.11"),
            Some("other-browser"),
        );

        assert_reuse_detected(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_reuse_session_treats_exact_grace_boundary_as_stale() {
        let now = Utc::now();
        let mut session = make_auth_session(now);
        session.last_seen_at = now - ChronoDuration::seconds(REFRESH_REUSE_GRACE_SECS);

        let result = classify_refresh_reuse_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            Some("203.0.113.10"),
            Some("browser"),
        );

        assert_stale_refresh(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_reuse_session_treats_just_outside_grace_as_reuse() {
        let now = Utc::now();
        let mut session = make_auth_session(now);
        session.last_seen_at = now - ChronoDuration::seconds(REFRESH_REUSE_GRACE_SECS + 1);

        let result = classify_refresh_reuse_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            Some("203.0.113.10"),
            Some("browser"),
        );

        assert_reuse_detected(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_reuse_session_treats_missing_client_evidence_as_reuse() {
        let now = Utc::now();
        let session = make_auth_session(now);

        let result =
            classify_refresh_reuse_session(Some(&session), 1, "refresh-jti", now, None, None);

        assert_reuse_detected(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_rotation_lock_loss_treats_recent_without_client_evidence_as_stale() {
        let now = Utc::now();
        let session = make_auth_session(now);

        let result = classify_refresh_rotation_lock_loss_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            None,
            None,
        );

        assert_stale_refresh(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_rotation_lock_loss_treats_just_outside_grace_as_reuse() {
        let now = Utc::now();
        let mut session = make_auth_session(now);
        session.last_seen_at = now - ChronoDuration::seconds(REFRESH_REUSE_GRACE_SECS + 1);

        let result = classify_refresh_rotation_lock_loss_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            None,
            None,
        );

        assert_reuse_detected(result, 1, "refresh-jti");
    }

    #[test]
    fn classify_refresh_rotation_lock_loss_treats_revoked_session_as_reuse() {
        let now = Utc::now();
        let mut session = make_auth_session(now);
        session.revoked_at = Some(now);

        let result = classify_refresh_rotation_lock_loss_session(
            Some(&session),
            1,
            "refresh-jti",
            now,
            None,
            None,
        );

        assert_reuse_detected(result, 1, "refresh-jti");
    }
}
