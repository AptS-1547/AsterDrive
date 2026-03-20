use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::db::repository::policy_repo;
use crate::entities::storage_policy;
use crate::errors::{AsterError, Result};
use crate::types::DriverType;

pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<storage_policy::Model>> {
    policy_repo::find_all(db).await
}

pub async fn get(db: &DatabaseConnection, id: i64) -> Result<storage_policy::Model> {
    policy_repo::find_by_id(db, id).await
}

pub async fn create(
    db: &DatabaseConnection,
    name: &str,
    driver_type: DriverType,
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    base_path: &str,
    max_file_size: i64,
    is_default: bool,
) -> Result<storage_policy::Model> {
    let now = Utc::now();
    let model = storage_policy::ActiveModel {
        name: Set(name.to_string()),
        driver_type: Set(driver_type),
        endpoint: Set(endpoint.to_string()),
        bucket: Set(bucket.to_string()),
        access_key: Set(access_key.to_string()),
        secret_key: Set(secret_key.to_string()),
        base_path: Set(base_path.to_string()),
        max_file_size: Set(max_file_size),
        allowed_types: Set("[]".to_string()),
        options: Set("{}".to_string()),
        is_default: Set(is_default),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    policy_repo::create(db, model).await
}

pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<()> {
    policy_repo::find_by_id(db, id).await?;
    storage_policy::Entity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}
