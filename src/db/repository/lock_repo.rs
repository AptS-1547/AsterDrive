use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entities::resource_lock::{self, Entity as ResourceLock};
use crate::errors::{AsterError, Result};
use crate::types::EntityType;

pub async fn create(
    db: &DatabaseConnection,
    model: resource_lock::ActiveModel,
) -> Result<resource_lock::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<resource_lock::Model>> {
    ResourceLock::find().all(db).await.map_err(AsterError::from)
}

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<Option<resource_lock::Model>> {
    ResourceLock::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::Token.eq(token))
        .one(db)
        .await
        .map_err(AsterError::from)
}

/// 查询单个资源的锁
pub async fn find_by_entity(
    db: &DatabaseConnection,
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
pub async fn find_by_path_prefix(
    db: &DatabaseConnection,
    prefix: &str,
) -> Result<Vec<resource_lock::Model>> {
    ResourceLock::find()
        .filter(resource_lock::Column::Path.starts_with(prefix))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 祖先路径查询（WebDAV check 用）
pub async fn find_ancestors(
    db: &DatabaseConnection,
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

pub async fn delete_by_id(db: &DatabaseConnection, id: i64) -> Result<()> {
    ResourceLock::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_by_token(db: &DatabaseConnection, token: &str) -> Result<()> {
    ResourceLock::delete_many()
        .filter(resource_lock::Column::Token.eq(token))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_by_entity(
    db: &DatabaseConnection,
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
pub async fn delete_by_path_prefix(db: &DatabaseConnection, prefix: &str) -> Result<u64> {
    let res = ResourceLock::delete_many()
        .filter(resource_lock::Column::Path.starts_with(prefix))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

/// 查找并返回所有过期锁
pub async fn find_expired(db: &DatabaseConnection) -> Result<Vec<resource_lock::Model>> {
    let now = Utc::now();
    ResourceLock::find()
        .filter(resource_lock::Column::TimeoutAt.is_not_null())
        .filter(resource_lock::Column::TimeoutAt.lt(now))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 删除过期锁（返回删除数量）
pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64> {
    let now = Utc::now();
    let res = ResourceLock::delete_many()
        .filter(resource_lock::Column::TimeoutAt.is_not_null())
        .filter(resource_lock::Column::TimeoutAt.lt(now))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(res.rows_affected)
}

pub async fn refresh(
    db: &DatabaseConnection,
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
