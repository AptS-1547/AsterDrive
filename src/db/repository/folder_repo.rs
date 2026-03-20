use crate::entities::folder::{self, Entity as Folder};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<folder::Model> {
    Folder::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("folder #{id}")))
}

pub async fn find_children(
    db: &DatabaseConnection,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .order_by_asc(folder::Column::Name);
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.all(db).await.map_err(AsterError::from)
}

pub async fn find_by_name_in_parent(
    db: &DatabaseConnection,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> Result<Option<folder::Model>> {
    let mut q = Folder::find()
        .filter(folder::Column::UserId.eq(user_id))
        .filter(folder::Column::Name.eq(name));
    q = match parent_id {
        Some(pid) => q.filter(folder::Column::ParentId.eq(pid)),
        None => q.filter(folder::Column::ParentId.is_null()),
    };
    q.one(db).await.map_err(AsterError::from)
}

pub async fn create(db: &DatabaseConnection, model: folder::ActiveModel) -> Result<folder::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    Folder::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}
