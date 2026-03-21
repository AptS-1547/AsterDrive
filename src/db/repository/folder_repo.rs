use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::folder::{self, Entity as Folder};
use crate::errors::{AsterError, Result};

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<folder::Model> {
    Folder::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))
}

/// 查询子文件夹（排除已删除）
pub async fn find_children(
    db: &DatabaseConnection,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::DeletedAt.is_null())
        .order_by_asc(folder::Column::Name);
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
}

/// 按名称查文件夹（排除已删除）
pub async fn find_by_name_in_parent(
    db: &DatabaseConnection,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::Name.eq(name))
        .filter(folder::Column::DeletedAt.is_null());
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
}

pub async fn create(db: &DatabaseConnection, model: folder::ActiveModel) -> Result<folder::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 硬删除文件夹记录（回收站清理用）
pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    Folder::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

// ── 软删除 / 回收站 ─────────────────────────────────────────────────

/// 软删除：标记 deleted_at
pub async fn soft_delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: folder::ActiveModel = f.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 恢复：清除 deleted_at
pub async fn restore(db: &DatabaseConnection, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: folder::ActiveModel = f.into();
    active.deleted_at = Set(None);
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 查询用户回收站中的文件夹（只查顶层被删除的，不含子目录）
pub async fn find_deleted_by_user(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::DeletedAt.is_not_null())
        .order_by_desc(folder::Column::DeletedAt)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询过期的已删除文件夹（自动清理用）
pub async fn find_expired_deleted(
    db: &DatabaseConnection,
    before: chrono::DateTime<Utc>,
) -> Result<Vec<folder::Model>> {
    Folder::find()
        .filter(folder::Column::DeletedAt.is_not_null())
        .filter(folder::Column::DeletedAt.lt(before))
        .all(db)
        .await
        .map_err(AsterError::from)
}
