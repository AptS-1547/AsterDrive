use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

use crate::db::repository::policy_repo;
use crate::entities::{storage_policy, user_storage_policy};
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

pub async fn update(
    db: &DatabaseConnection,
    id: i64,
    name: Option<String>,
    endpoint: Option<String>,
    bucket: Option<String>,
    access_key: Option<String>,
    secret_key: Option<String>,
    base_path: Option<String>,
    max_file_size: Option<i64>,
    is_default: Option<bool>,
) -> Result<storage_policy::Model> {
    let existing = policy_repo::find_by_id(db, id).await?;
    let mut active: storage_policy::ActiveModel = existing.into();
    if let Some(v) = name {
        active.name = Set(v);
    }
    if let Some(v) = endpoint {
        active.endpoint = Set(v);
    }
    if let Some(v) = bucket {
        active.bucket = Set(v);
    }
    if let Some(v) = access_key {
        active.access_key = Set(v);
    }
    if let Some(v) = secret_key {
        active.secret_key = Set(v);
    }
    if let Some(v) = base_path {
        active.base_path = Set(v);
    }
    if let Some(v) = max_file_size {
        active.max_file_size = Set(v);
    }
    if let Some(v) = is_default {
        active.is_default = Set(v);
    }
    active.updated_at = Set(Utc::now());
    active.update(db).await.map_err(AsterError::from)
}

// ── User Storage Policy ──────────────────────────────────────────────

pub async fn list_user_policies(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<user_storage_policy::Model>> {
    policy_repo::find_user_policies(db, user_id).await
}

pub async fn assign_user_policy(
    db: &DatabaseConnection,
    user_id: i64,
    policy_id: i64,
    is_default: bool,
    quota_bytes: i64,
) -> Result<user_storage_policy::Model> {
    // 校验策略存在
    policy_repo::find_by_id(db, policy_id).await?;

    // 如果设为默认，先清除该用户的其他默认
    if is_default {
        policy_repo::clear_user_default(db, user_id).await?;
    }

    let model = user_storage_policy::ActiveModel {
        user_id: Set(user_id),
        policy_id: Set(policy_id),
        is_default: Set(is_default),
        quota_bytes: Set(quota_bytes),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    policy_repo::create_user_policy(db, model).await
}

pub async fn update_user_policy(
    db: &DatabaseConnection,
    id: i64,
    is_default: Option<bool>,
    quota_bytes: Option<i64>,
) -> Result<user_storage_policy::Model> {
    let existing = policy_repo::find_user_policy_by_id(db, id).await?;

    // 如果设为默认，先清除该用户的其他默认
    if let Some(true) = is_default {
        policy_repo::clear_user_default(db, existing.user_id).await?;
    }

    let mut active: user_storage_policy::ActiveModel = existing.into();
    if let Some(v) = is_default {
        active.is_default = Set(v);
    }
    if let Some(v) = quota_bytes {
        active.quota_bytes = Set(v);
    }
    policy_repo::update_user_policy(db, active).await
}

pub async fn remove_user_policy(db: &DatabaseConnection, id: i64) -> Result<()> {
    // 校验存在
    policy_repo::find_user_policy_by_id(db, id).await?;
    policy_repo::delete_user_policy(db, id).await
}
