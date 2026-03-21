use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::{
    file::{self, Entity as File},
    file_blob::{self, Entity as FileBlob},
};
use crate::errors::{AsterError, Result};

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<file::Model> {
    File::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::file_not_found(format!("file #{id}")))
}

/// 查询文件夹下的文件（排除已删除）
pub async fn find_by_folder(
    db: &DatabaseConnection,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::DeletedAt.is_null())
        .order_by_asc(file::Column::Name);
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
}

/// 按名称查文件（排除已删除）
pub async fn find_by_name_in_folder(
    db: &DatabaseConnection,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::Name.eq(name))
        .filter(file::Column::DeletedAt.is_null());
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
}

pub async fn find_blob_by_hash(
    db: &DatabaseConnection,
    hash: &str,
    policy_id: i64,
) -> Result<Option<file_blob::Model>> {
    FileBlob::find()
        .filter(file_blob::Column::Hash.eq(hash))
        .filter(file_blob::Column::PolicyId.eq(policy_id))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create_blob(
    db: &DatabaseConnection,
    model: file_blob::ActiveModel,
) -> Result<file_blob::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn create(db: &DatabaseConnection, model: file::ActiveModel) -> Result<file::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn find_blob_by_id(db: &DatabaseConnection, id: i64) -> Result<file_blob::Model> {
    FileBlob::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("file_blob #{id}")))
}

/// 批量查询 blob，返回 id → Model 的映射
pub async fn find_blobs_by_ids(
    db: &DatabaseConnection,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, file_blob::Model>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let blobs = FileBlob::find()
        .filter(file_blob::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok(blobs.into_iter().map(|b| (b.id, b)).collect())
}

/// 硬删除文件记录（回收站清理用）
pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    File::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn delete_blob(db: &DatabaseConnection, id: i64) -> Result<()> {
    FileBlob::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

// ── 软删除 / 回收站 ─────────────────────────────────────────────────

/// 软删除：标记 deleted_at
pub async fn soft_delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 恢复：清除 deleted_at
pub async fn restore(db: &DatabaseConnection, id: i64) -> Result<()> {
    let f = find_by_id(db, id).await?;
    let mut active: file::ActiveModel = f.into();
    active.deleted_at = Set(None);
    active.update(db).await.map_err(AsterError::from)?;
    Ok(())
}

/// 查询用户回收站中的文件
pub async fn find_deleted_by_user(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::DeletedAt.is_not_null())
        .order_by_desc(file::Column::DeletedAt)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 查询过期的已删除文件（自动清理用）
pub async fn find_expired_deleted(
    db: &DatabaseConnection,
    before: chrono::DateTime<Utc>,
) -> Result<Vec<file::Model>> {
    File::find()
        .filter(file::Column::DeletedAt.is_not_null())
        .filter(file::Column::DeletedAt.lt(before))
        .all(db)
        .await
        .map_err(AsterError::from)
}
