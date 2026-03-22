use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

use crate::entities::audit_log::{self, Entity as AuditLog};
use crate::errors::{AsterError, Result};

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: audit_log::ActiveModel,
) -> Result<audit_log::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 带过滤条件的分页查询
pub async fn find_with_filters<C: ConnectionTrait>(
    db: &C,
    user_id: Option<i64>,
    action: Option<&str>,
    entity_type: Option<&str>,
    after: Option<DateTime<Utc>>,
    before: Option<DateTime<Utc>>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<audit_log::Model>, u64)> {
    let mut q = AuditLog::find().order_by_desc(audit_log::Column::CreatedAt);

    if let Some(uid) = user_id {
        q = q.filter(audit_log::Column::UserId.eq(uid));
    }
    if let Some(act) = action {
        q = q.filter(audit_log::Column::Action.eq(act));
    }
    if let Some(et) = entity_type {
        q = q.filter(audit_log::Column::EntityType.eq(et));
    }
    if let Some(after) = after {
        q = q.filter(audit_log::Column::CreatedAt.gte(after));
    }
    if let Some(before) = before {
        q = q.filter(audit_log::Column::CreatedAt.lte(before));
    }

    let total = q.clone().count(db).await.map_err(AsterError::from)?;
    let items = q
        .limit(limit)
        .offset(offset)
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
