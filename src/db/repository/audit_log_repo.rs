//! 仓储模块：`audit_log_repo`。

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::entities::audit_log::{self, Entity as AuditLog};
use crate::errors::{AsterError, Result};

pub struct AuditLogQuery<'a> {
    pub user_id: Option<i64>,
    pub action: Option<&'a str>,
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<i64>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: u64,
    pub offset: u64,
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: audit_log::ActiveModel,
) -> Result<audit_log::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn create_many<C: ConnectionTrait>(
    db: &C,
    models: Vec<audit_log::ActiveModel>,
) -> Result<()> {
    if models.is_empty() {
        return Ok(());
    }
    AuditLog::insert_many(models)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 带过滤条件的分页查询
pub async fn find_with_filters<C: ConnectionTrait>(
    db: &C,
    query: AuditLogQuery<'_>,
) -> Result<(Vec<audit_log::Model>, u64)> {
    let mut q = AuditLog::find()
        .order_by_desc(audit_log::Column::CreatedAt)
        .order_by_desc(audit_log::Column::Id);

    if let Some(uid) = query.user_id {
        q = q.filter(audit_log::Column::UserId.eq(uid));
    }
    if let Some(act) = query.action {
        q = q.filter(audit_log::Column::Action.eq(act));
    }
    if let Some(et) = query.entity_type {
        q = q.filter(audit_log::Column::EntityType.eq(et));
    }
    if let Some(eid) = query.entity_id {
        q = q.filter(audit_log::Column::EntityId.eq(eid));
    }
    if let Some(after) = query.after {
        q = q.filter(audit_log::Column::CreatedAt.gte(after));
    }
    if let Some(before) = query.before {
        q = q.filter(audit_log::Column::CreatedAt.lte(before));
    }

    let total = q.clone().count(db).await.map_err(AsterError::from)?;
    let items = q
        .limit(query.limit)
        .offset(query.offset)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok((items, total))
}

/// 删除指定时间之前的审计日志
pub async fn delete_before<C: ConnectionTrait>(db: &C, before: DateTime<Utc>) -> Result<u64> {
    let res = AuditLog::delete_many()
        .filter(audit_log::Column::CreatedAt.lt(before))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

/// 查询指定时间范围内的日志 action 和 created_at（用于管理后台每日统计）
pub async fn find_actions_in_range<C: ConnectionTrait>(
    db: &C,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<(String, DateTime<Utc>)>> {
    AuditLog::find()
        .select_only()
        .column(audit_log::Column::Action)
        .column(audit_log::Column::CreatedAt)
        .filter(audit_log::Column::CreatedAt.gte(start))
        .filter(audit_log::Column::CreatedAt.lt(end))
        .into_tuple::<(String, DateTime<Utc>)>()
        .all(db)
        .await
        .map_err(AsterError::from)
}
