//! 仓储模块：`lock_repo`。

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TryInsertResult,
};

use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::resource_lock::{self, Entity as ResourceLock};
use crate::errors::{AsterError, Result};
use crate::types::EntityType;

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: resource_lock::ActiveModel,
) -> Result<resource_lock::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn create_if_unlocked<C: ConnectionTrait>(
    db: &C,
    model: resource_lock::ActiveModel,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<Option<resource_lock::Model>> {
    let inserted = match ResourceLock::insert(model)
        .on_conflict_do_nothing_on([
            resource_lock::Column::EntityType,
            resource_lock::Column::EntityId,
        ])
        .exec(db)
        .await
        .map_err(AsterError::from)?
    {
        TryInsertResult::Inserted(_) => true,
        TryInsertResult::Conflicted => false,
        TryInsertResult::Empty => {
            return Err(AsterError::internal_error(
                "resource lock insert produced empty result",
            ));
        }
    };

    if !inserted {
        return Ok(None);
    }

    find_by_entity(db, entity_type, entity_id)
        .await?
        .map(Some)
        .ok_or_else(|| {
            AsterError::internal_error(format!(
                "resource lock insert could not reload entity lock for {entity_type:?}#{entity_id}"
            ))
        })
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<resource_lock::Model>> {
    ResourceLock::find()
        .order_by_asc(resource_lock::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
) -> Result<(Vec<resource_lock::Model>, u64)> {
    fetch_offset_page(
        db,
        ResourceLock::find().order_by_asc(resource_lock::Column::Id),
        limit,
        offset,
    )
    .await
}

pub async fn find_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<resource_lock::Model>> {
    ResourceLock::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_token<C: ConnectionTrait>(
    db: &C,
    token: &str,
) -> Result<Option<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::Token.eq(token))
        .one(db)
        .await
        .map_err(AsterError::from)
}

/// 查询单个资源的锁
pub async fn find_by_entity<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<Option<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::EntityType.eq(entity_type))
        .filter(resource_lock::Column::EntityId.eq(entity_id))
        .one(db)
        .await
        .map_err(AsterError::from)
}

/// 路径前缀查询（WebDAV deep lock 用）
pub async fn find_by_path_prefix<C: ConnectionTrait>(
    db: &C,
    prefix: &str,
) -> Result<Vec<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::Path.starts_with(prefix))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 祖先路径查询（WebDAV check 用）
pub async fn find_ancestors<C: ConnectionTrait>(
    db: &C,
    paths: &[String],
) -> Result<Vec<resource_lock::Model>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }
    ResourceLock::find()
        .filter(resource_lock::Column::Path.is_in(paths.iter().map(|s| s.as_str())))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    ResourceLock::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_by_token<C: ConnectionTrait>(db: &C, token: &str) -> Result<()> {
    ResourceLock::delete_many()
        .filter(resource_lock::Column::Token.eq(token))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_by_entity<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<()> {
    ResourceLock::delete_many()
        .filter(resource_lock::Column::EntityType.eq(entity_type))
        .filter(resource_lock::Column::EntityId.eq(entity_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 删除路径前缀匹配的所有锁
pub async fn delete_by_path_prefix<C: ConnectionTrait>(db: &C, prefix: &str) -> Result<u64> {
    let res = ResourceLock::delete_many()
        .filter(resource_lock::Column::Path.starts_with(prefix))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

/// 查找并返回所有过期锁
pub async fn find_expired<C: ConnectionTrait>(db: &C) -> Result<Vec<resource_lock::Model>> {
    let now = Utc::now();
    ResourceLock::find()
        .filter(resource_lock::Column::TimeoutAt.is_not_null())
        .filter(resource_lock::Column::TimeoutAt.lt(now))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 删除过期锁（返回删除数量）
pub async fn delete_expired<C: ConnectionTrait>(db: &C) -> Result<u64> {
    let now = Utc::now();
    let res = ResourceLock::delete_many()
        .filter(resource_lock::Column::TimeoutAt.is_not_null())
        .filter(resource_lock::Column::TimeoutAt.lt(now))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

pub async fn refresh<C: ConnectionTrait>(
    db: &C,
    token: &str,
    new_timeout_at: Option<chrono::DateTime<Utc>>,
) -> Result<Option<resource_lock::Model>> {
    let lock = find_by_token(db, token).await?;
    match lock {
        Some(l) => {
            let mut active: resource_lock::ActiveModel = l.into();
            active.timeout_at = Set(new_timeout_at);
            let updated = active.update(db).await.map_err(AsterError::from)?;
            Ok(Some(updated))
        }
        None => Ok(None),
    }
}

/// 查询用户持有的所有资源锁
pub async fn find_by_owner<C: ConnectionTrait>(
    db: &C,
    owner_id: i64,
) -> Result<Vec<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 批量删除用户持有的所有资源锁
pub async fn delete_all_by_owner<C: ConnectionTrait>(db: &C, owner_id: i64) -> Result<u64> {
    let res = ResourceLock::delete_many()
        .filter(resource_lock::Column::OwnerId.eq(owner_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sea_orm::{DbBackend, QueryTrait};

    #[test]
    fn postgres_create_if_unlocked_sql_uses_entity_conflict_target() {
        let sql = ResourceLock::insert(resource_lock::ActiveModel {
            token: Set("urn:uuid:test".to_string()),
            entity_type: Set(EntityType::File),
            entity_id: Set(7),
            path: Set("/docs/report.txt".to_string()),
            owner_id: Set(Some(9)),
            owner_info: Set(None),
            timeout_at: Set(None),
            shared: Set(false),
            deep: Set(false),
            created_at: Set(Utc::now()),
            ..Default::default()
        })
        .on_conflict_do_nothing_on([
            resource_lock::Column::EntityType,
            resource_lock::Column::EntityId,
        ])
        .build(DbBackend::Postgres)
        .to_string();

        assert!(
            sql.contains(r#"ON CONFLICT ("entity_type", "entity_id") DO NOTHING"#),
            "{sql}"
        );
        assert!(!sql.contains(" WHERE "), "{sql}");
    }
}
