//! 仓储模块：`policy_repo`。

use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::storage_policy::{self, Entity as StoragePolicy};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, sea_query::Expr,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<storage_policy::Model> {
    StoragePolicy::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::storage_policy_not_found(format!("policy #{id}")))
}

pub async fn lock_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<storage_policy::Model> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => StoragePolicy::find_by_id(id)
            .lock_exclusive()
            .one(db)
            .await
            .map_err(AsterError::from)?
            .ok_or_else(|| AsterError::storage_policy_not_found(format!("policy #{id}"))),
        DbBackend::Sqlite => find_by_id(db, id).await,
        _ => find_by_id(db, id).await,
    }
}

pub async fn find_default<C: ConnectionTrait>(db: &C) -> Result<Option<storage_policy::Model>> {
    StoragePolicy::find()
        .filter(storage_policy::Column::IsDefault.eq(true))
        .order_by_asc(storage_policy::Column::Id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<storage_policy::Model>> {
    StoragePolicy::find()
        .order_by_asc(storage_policy::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
) -> Result<(Vec<storage_policy::Model>, u64)> {
    fetch_offset_page(
        db,
        StoragePolicy::find()
            .order_by_desc(storage_policy::Column::CreatedAt)
            .order_by_desc(storage_policy::Column::Id),
        limit,
        offset,
    )
    .await
}

pub async fn count_by_remote_node_id<C: ConnectionTrait>(
    db: &C,
    remote_node_id: i64,
) -> Result<u64> {
    StoragePolicy::find()
        .filter(storage_policy::Column::RemoteNodeId.eq(remote_node_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: storage_policy::ActiveModel,
) -> Result<storage_policy::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 清除所有系统策略的 is_default（新 default 设置前调用）
pub async fn clear_system_default<C: ConnectionTrait>(db: &C) -> Result<()> {
    let defaults = StoragePolicy::find()
        .filter(storage_policy::Column::IsDefault.eq(true))
        .all(db)
        .await
        .map_err(AsterError::from)?;
    for m in defaults {
        let mut active: storage_policy::ActiveModel = m.into();
        active.is_default = Set(false);
        active.update(db).await.map_err(AsterError::from)?;
    }
    Ok(())
}

pub async fn set_only_default<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    find_by_id(db, id).await?;

    StoragePolicy::update_many()
        .col_expr(
            storage_policy::Column::IsDefault,
            Expr::case(Expr::col(storage_policy::Column::Id).eq(id), true)
                .finally(false)
                .into(),
        )
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let result = StoragePolicy::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    if result.rows_affected == 0 {
        return Err(AsterError::storage_policy_not_found(format!(
            "policy #{id}"
        )));
    }
    Ok(())
}
