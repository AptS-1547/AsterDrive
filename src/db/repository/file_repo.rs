use crate::entities::{
    file::{self, Entity as File},
    file_blob::{self, Entity as FileBlob},
};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<file::Model> {
    File::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::file_not_found(format!("file #{id}")))
}

pub async fn find_by_folder(
    db: &DatabaseConnection,
    user_id: i64,
    folder_id: Option<i64>,
) -> Result<Vec<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .order_by_asc(file::Column::Name);
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
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

pub async fn find_by_name_in_folder(
    db: &DatabaseConnection,
    user_id: i64,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    let mut q = File::find()
        .filter(file::Column::UserId.eq(user_id))
        .filter(file::Column::Name.eq(name));
    q = match folder_id {
        Some(fid) => q.filter(file::Column::FolderId.eq(fid)),
        None => q.filter(file::Column::FolderId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
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
