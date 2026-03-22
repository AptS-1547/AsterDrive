use actix_web::HttpRequest;
use chrono::{Duration, Utc};
use sea_orm::Set;
use serde::Serialize;
use utoipa::ToSchema;

use crate::db::repository::{audit_log_repo, config_repo};
use crate::entities::audit_log;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::auth_service::Claims;

const DEFAULT_RETENTION_DAYS: i64 = 90;

/// 从 HttpRequest 提取的审计上下文
pub struct AuditContext {
    pub user_id: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditContext {
    pub fn from_request(req: &HttpRequest, claims: &Claims) -> Self {
        let ip_address = req
            .connection_info()
            .realip_remote_addr()
            .map(|s| s.to_string());
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Self {
            user_id: claims.user_id,
            ip_address,
            user_agent,
        }
    }
}

/// Fire-and-forget 审计日志。DB 错误只 warn 不传播。
pub async fn log(
    state: &AppState,
    ctx: &AuditContext,
    action: &str,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
    entity_name: Option<&str>,
    details: Option<serde_json::Value>,
) {
    // 检查运行时配置
    match config_repo::find_by_key(&state.db, "audit_log_enabled").await {
        Ok(Some(cfg)) if cfg.value == "false" => return,
        Err(e) => {
            tracing::warn!("failed to check audit_log_enabled: {e}");
            // 读不到配置就默认启用，继续记录
        }
        _ => {}
    }

    let model = audit_log::ActiveModel {
        id: Default::default(),
        user_id: Set(ctx.user_id),
        action: Set(action.to_string()),
        entity_type: Set(entity_type.map(|s| s.to_string())),
        entity_id: Set(entity_id),
        entity_name: Set(entity_name.map(|s| s.to_string())),
        details: Set(details.map(|v| v.to_string())),
        ip_address: Set(ctx.ip_address.clone()),
        user_agent: Set(ctx.user_agent.clone()),
        created_at: Set(Utc::now()),
    };

    if let Err(e) = audit_log_repo::create(&state.db, model).await {
        tracing::warn!("failed to write audit log: {e}");
    }
}

/// Admin 分页查询
#[derive(Serialize, ToSchema)]
pub struct AuditLogPage {
    pub items: Vec<audit_log::Model>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

pub async fn query(
    state: &AppState,
    user_id: Option<i64>,
    action: Option<&str>,
    entity_type: Option<&str>,
    after: Option<chrono::DateTime<Utc>>,
    before: Option<chrono::DateTime<Utc>>,
    limit: u64,
    offset: u64,
) -> Result<AuditLogPage> {
    let limit = limit.min(200).max(1);
    let (items, total) = audit_log_repo::find_with_filters(
        &state.db,
        user_id,
        action,
        entity_type,
        after,
        before,
        limit,
        offset,
    )
    .await?;

    Ok(AuditLogPage {
        items,
        total,
        limit,
        offset,
    })
}

/// 清理过期审计日志
pub async fn cleanup_expired(state: &AppState) -> Result<u64> {
    let retention_days =
        match config_repo::find_by_key(&state.db, "audit_log_retention_days").await? {
            Some(cfg) => cfg.value.parse::<i64>().unwrap_or_else(|_| {
                tracing::warn!(
                    "invalid audit_log_retention_days value '{}', using default",
                    cfg.value
                );
                DEFAULT_RETENTION_DAYS
            }),
            None => DEFAULT_RETENTION_DAYS,
        };

    let cutoff = Utc::now() - Duration::days(retention_days);
    let deleted = audit_log_repo::delete_before(&state.db, cutoff).await?;
    if deleted > 0 {
        tracing::info!("cleaned up {deleted} expired audit log entries");
    }
    Ok(deleted)
}
