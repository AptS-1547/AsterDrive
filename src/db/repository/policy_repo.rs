use crate::entities::{
    storage_policy::{self, Entity as StoragePolicy},
    user_storage_policy::{self, Entity as UserStoragePolicy},
};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};

pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<storage_policy::Model> {
    StoragePolicy::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::storage_policy_not_found(format!("policy #{id}")))
}

pub async fn find_default(db: &DatabaseConnection) -> Result<Option<storage_policy::Model>> {
    StoragePolicy::find()
        .filter(storage_policy::Column::IsDefault.eq(true))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<storage_policy::Model>> {
    StoragePolicy::find()
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_user_default(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Option<user_storage_policy::Model>> {
    UserStoragePolicy::find()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .filter(user_storage_policy::Column::IsDefault.eq(true))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create(
    db: &DatabaseConnection,
    model: storage_policy::ActiveModel,
) -> Result<storage_policy::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

// ── User Storage Policy ──────────────────────────────────────────────

pub async fn find_user_policies(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<user_storage_policy::Model>> {
    UserStoragePolicy::find()
        .filter(user_storage_policy::Column::UserId.eq(user_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_user_policy_by_id(
    db: &DatabaseConnection,
    id: i64,
) -> Result<user_storage_policy::Model> {
    UserStoragePolicy::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("user_storage_policy #{id}")))
}

/// 清除用户的其他默认策略（设 is_default=false）
pub async fn clear_user_default(db: &DatabaseConnection, user_id: i64) -> Result<()> {
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

pub async fn create_user_policy(
    db: &DatabaseConnection,
    model: user_storage_policy::ActiveModel,
) -> Result<user_storage_policy::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update_user_policy(
    db: &DatabaseConnection,
    model: user_storage_policy::ActiveModel,
) -> Result<user_storage_policy::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn delete_user_policy(db: &DatabaseConnection, id: i64) -> Result<()> {
    UserStoragePolicy::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}
