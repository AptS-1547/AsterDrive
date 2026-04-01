use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::{
    storage_policy::{self, Entity as StoragePolicy},
    user_storage_policy::{self, Entity as UserStoragePolicy},
};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter,
    QueryOrder, Set, sea_query::Expr,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<storage_policy::Model> {
    StoragePolicy::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::storage_policy_not_found(format!("policy #{id}")))
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
        StoragePolicy::find().order_by_asc(storage_policy::Column::Id),
        limit,
        offset,
    )
    .await
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn find_user_default<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Option<user_storage_policy::Model>> {
    UserStoragePolicy::find()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .filter(user_storage_policy::Column::IsDefault.eq(true))
        .one(db)
        .await
        .map_err(AsterError::from)
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn find_all_user_defaults<C: ConnectionTrait>(
    db: &C,
) -> Result<Vec<user_storage_policy::Model>> {
    UserStoragePolicy::find()
        .filter(user_storage_policy::Column::IsDefault.eq(true))
        .all(db)
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

// ── User Storage Policy ──────────────────────────────────────────────

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn find_user_policies<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<user_storage_policy::Model>> {
    UserStoragePolicy::find()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .order_by_asc(user_storage_policy::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn find_user_policies_paginated<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<user_storage_policy::Model>, u64)> {
    fetch_offset_page(
        db,
        UserStoragePolicy::find()
            .filter(user_storage_policy::Column::UserId.eq(user_id))
            .order_by_asc(user_storage_policy::Column::Id),
        limit,
        offset,
    )
    .await
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn find_user_policy_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<user_storage_policy::Model> {
    UserStoragePolicy::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("user_storage_policy #{id}")))
}

/// 清除用户的其他默认策略（设 is_default=false）
#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn clear_user_default<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<()> {
    use sea_orm::QueryFilter;
    let existing = UserStoragePolicy::find()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .filter(user_storage_policy::Column::IsDefault.eq(true))
        .all(db)
        .await
        .map_err(AsterError::from)?;

    for m in existing {
        let mut active: user_storage_policy::ActiveModel = m.into();
        active.is_default = Set(false);
        active.update(db).await.map_err(AsterError::from)?;
    }
    Ok(())
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn create_user_policy<C: ConnectionTrait>(
    db: &C,
    model: user_storage_policy::ActiveModel,
) -> Result<user_storage_policy::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn update_user_policy<C: ConnectionTrait>(
    db: &C,
    model: user_storage_policy::ActiveModel,
) -> Result<user_storage_policy::Model> {
    model.update(db).await.map_err(AsterError::from)
}

#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn delete_user_policy<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    UserStoragePolicy::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量删除用户的所有存储策略分配
#[deprecated(
    since = "0.1.0",
    note = "legacy user_storage_policy compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn delete_user_policies_by_user<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<u64> {
    let res = UserStoragePolicy::delete_many()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}
