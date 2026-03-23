use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use crate::entities::file_version::{self, Entity as FileVersion};
use crate::errors::{AsterError, Result};

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: file_version::ActiveModel,
) -> Result<file_version::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 按 file_id 查询所有版本（version DESC）
pub async fn find_by_file_id<C: ConnectionTrait>(
    db: &C,
    file_id: i64,
) -> Result<Vec<file_version::Model>> {
    FileVersion::find()
        .filter(file_version::Column::FileId.eq(file_id))
        .order_by_desc(file_version::Column::Version)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<file_version::Model>> {
    FileVersion::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    FileVersion::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 查找指定版本及之后的所有版本（version DESC）
pub async fn find_by_file_id_from_version<C: ConnectionTrait>(
    db: &C,
    file_id: i64,
    min_version: i32,
) -> Result<Vec<file_version::Model>> {
    FileVersion::find()
        .filter(file_version::Column::FileId.eq(file_id))
        .filter(file_version::Column::Version.gte(min_version))
        .order_by_desc(file_version::Column::Version)
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 删除指定版本及之后的所有版本，返回对应 blob_id 列表
pub async fn delete_by_file_id_from_version<C: ConnectionTrait>(
    db: &C,
    file_id: i64,
    min_version: i32,
) -> Result<Vec<i64>> {
    let versions = find_by_file_id_from_version(db, file_id, min_version).await?;
    let blob_ids: Vec<i64> = versions.iter().map(|v| v.blob_id).collect();

    FileVersion::delete_many()
        .filter(file_version::Column::FileId.eq(file_id))
        .filter(file_version::Column::Version.gte(min_version))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    Ok(blob_ids)
}

/// 统计文件的版本数量
pub async fn count_by_file_id<C: ConnectionTrait>(db: &C, file_id: i64) -> Result<u64> {
    FileVersion::find()
        .filter(file_version::Column::FileId.eq(file_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

/// 查找最旧的版本（version ASC limit 1）
pub async fn find_oldest_by_file_id<C: ConnectionTrait>(
    db: &C,
    file_id: i64,
) -> Result<Option<file_version::Model>> {
    FileVersion::find()
        .filter(file_version::Column::FileId.eq(file_id))
        .order_by_asc(file_version::Column::Version)
        .one(db)
        .await
        .map_err(AsterError::from)
}

/// 删除文件的所有版本记录（文件永久删除时用）
pub async fn delete_all_by_file_id<C: ConnectionTrait>(db: &C, file_id: i64) -> Result<Vec<i64>> {
    // 先查出所有 blob_id（需要减引用计数）
    let versions = find_by_file_id(db, file_id).await?;
    let blob_ids: Vec<i64> = versions.iter().map(|v| v.blob_id).collect();

    FileVersion::delete_many()
        .filter(file_version::Column::FileId.eq(file_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    Ok(blob_ids)
}

/// 获取下一个版本号
pub async fn next_version<C: ConnectionTrait>(db: &C, file_id: i64) -> Result<i32> {
    let latest = FileVersion::find()
        .filter(file_version::Column::FileId.eq(file_id))
        .order_by_desc(file_version::Column::Version)
        .one(db)
        .await
        .map_err(AsterError::from)?;
    Ok(latest.map(|v| v.version + 1).unwrap_or(1))
}
