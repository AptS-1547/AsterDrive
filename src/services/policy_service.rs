use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::db::repository::policy_repo;
use crate::entities::{storage_policy, user_storage_policy};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::types::DriverType;

pub async fn list_all(state: &AppState) -> Result<Vec<storage_policy::Model>> {
    policy_repo::find_all(&state.db).await
}

pub async fn get(state: &AppState, id: i64) -> Result<storage_policy::Model> {
    policy_repo::find_by_id(&state.db, id).await
}

pub async fn create(
    state: &AppState,
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
    // 设为默认时清除其他策略的 default
    if is_default {
        policy_repo::clear_system_default(&state.db).await?;
    }

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
        chunk_size: Set(5_242_880), // 5MB default
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let result = policy_repo::create(&state.db, model).await?;
    if is_default {
        invalidate_policy_cache(state, result.id).await;
    }
    Ok(result)
}

pub async fn delete(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;

    // 不允许删除唯一的默认策略
    if policy.is_default {
        let all = policy_repo::find_all(&state.db).await?;
        let default_count = all.iter().filter(|p| p.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot delete the only default storage policy",
            ));
        }
    }

    // 引用保护：有 blob 引用则拒绝删除（blob 的物理文件依赖此策略的存储驱动）
    let blob_count = crate::db::repository::file_repo::count_blobs_by_policy(&state.db, id).await?;
    if blob_count > 0 {
        return Err(AsterError::validation_error(format!(
            "cannot delete policy: {blob_count} blob(s) still reference it"
        )));
    }

    // 清除引用此策略的文件夹覆盖设置
    let cleared =
        crate::db::repository::folder_repo::clear_policy_references(&state.db, id).await?;
    if cleared > 0 {
        tracing::info!("cleared policy_id on {cleared} folders before deleting policy #{id}");
    }

    storage_policy::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(AsterError::from)?;

    invalidate_policy_cache(state, id).await;
    state.driver_registry.invalidate(id);
    Ok(())
}

pub async fn update(
    state: &AppState,
    id: i64,
    name: Option<String>,
    endpoint: Option<String>,
    bucket: Option<String>,
    access_key: Option<String>,
    secret_key: Option<String>,
    base_path: Option<String>,
    max_file_size: Option<i64>,
    chunk_size: Option<i64>,
    is_default: Option<bool>,
    options: Option<String>,
) -> Result<storage_policy::Model> {
    let existing = policy_repo::find_by_id(&state.db, id).await?;

    // 不允许取消唯一的系统默认策略
    if let Some(false) = is_default
        && existing.is_default
        && policy_repo::find_default(&state.db).await?.is_some()
    {
        // 检查是否是唯一的 default
        let all = policy_repo::find_all(&state.db).await?;
        let default_count = all.iter().filter(|p| p.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot unset the only default storage policy",
            ));
        }
    }

    // 设为默认时清除其他
    if let Some(true) = is_default {
        policy_repo::clear_system_default(&state.db).await?;
    }

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
    if let Some(v) = chunk_size {
        active.chunk_size = Set(v);
    }
    if let Some(v) = is_default {
        active.is_default = Set(v);
    }
    if let Some(v) = options {
        active.options = Set(v);
    }
    active.updated_at = Set(Utc::now());
    let result = active.update(&state.db).await.map_err(AsterError::from)?;

    // 缓存失效：策略数据 + driver 实例
    invalidate_policy_cache(state, id).await;
    state.driver_registry.invalidate(id);

    Ok(result)
}

/// 测试存储策略连接是否正常
///
/// - Local: 检查 base_path 目录是否可写
/// - S3: 尝试 HEAD bucket
pub async fn test_connection(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    // 写一个测试文件然后删除
    let test_path = "_aster_connection_test";
    driver
        .put(test_path, b"ok")
        .await
        .map_err(|e| AsterError::storage_driver_error(format!("write test failed: {e}")))?;
    let _ = driver.delete(test_path).await;

    Ok(())
}

/// 测试存储策略连接（不保存，用临时构造的 policy）
pub async fn test_connection_params(
    driver_type: DriverType,
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    base_path: &str,
) -> Result<()> {
    use crate::entities::storage_policy;
    use crate::storage::local::LocalDriver;
    use crate::storage::s3::S3Driver;

    // 构造一个临时 policy model 用于创建 driver
    let fake_policy = storage_policy::Model {
        id: 0,
        name: String::new(),
        driver_type,
        endpoint: endpoint.to_string(),
        bucket: bucket.to_string(),
        access_key: access_key.to_string(),
        secret_key: secret_key.to_string(),
        base_path: base_path.to_string(),
        max_file_size: 0,
        allowed_types: String::new(),
        options: String::new(),
        is_default: false,
        chunk_size: 0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let driver: Box<dyn crate::storage::driver::StorageDriver> = match driver_type {
        DriverType::Local => Box::new(LocalDriver::new(&fake_policy)?),
        DriverType::S3 => Box::new(S3Driver::new(&fake_policy)?),
    };

    let test_path = "_aster_connection_test";
    driver
        .put(test_path, b"ok")
        .await
        .map_err(|e| AsterError::storage_driver_error(format!("connection test failed: {e}")))?;
    let _ = driver.delete(test_path).await;

    Ok(())
}

// ── User Storage Policy ──────────────────────────────────────────────

pub async fn list_user_policies(
    state: &AppState,
    user_id: i64,
) -> Result<Vec<user_storage_policy::Model>> {
    policy_repo::find_user_policies(&state.db, user_id).await
}

pub async fn assign_user_policy(
    state: &AppState,
    user_id: i64,
    policy_id: i64,
    is_default: bool,
    quota_bytes: i64,
) -> Result<user_storage_policy::Model> {
    // 校验策略存在
    policy_repo::find_by_id(&state.db, policy_id).await?;

    // 如果设为默认，先清除该用户的其他默认
    if is_default {
        policy_repo::clear_user_default(&state.db, user_id).await?;
    }

    let model = user_storage_policy::ActiveModel {
        user_id: Set(user_id),
        policy_id: Set(policy_id),
        is_default: Set(is_default),
        quota_bytes: Set(quota_bytes),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    let result = policy_repo::create_user_policy(&state.db, model).await?;

    // invalidate cache
    state
        .cache
        .delete(&format!("user_default_policy:{user_id}"))
        .await;

    Ok(result)
}

pub async fn update_user_policy(
    state: &AppState,
    id: i64,
    is_default: Option<bool>,
    quota_bytes: Option<i64>,
) -> Result<user_storage_policy::Model> {
    let existing = policy_repo::find_user_policy_by_id(&state.db, id).await?;

    // 不允许取消唯一的用户默认策略
    if let Some(false) = is_default
        && existing.is_default
    {
        let user_policies = policy_repo::find_user_policies(&state.db, existing.user_id).await?;
        let default_count = user_policies.iter().filter(|p| p.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot unset the only default user policy",
            ));
        }
    }

    // 如果设为默认，先清除该用户的其他默认
    if let Some(true) = is_default {
        policy_repo::clear_user_default(&state.db, existing.user_id).await?;
    }

    let user_id = existing.user_id;
    let mut active: user_storage_policy::ActiveModel = existing.into();
    if let Some(v) = is_default {
        active.is_default = Set(v);
    }
    if let Some(v) = quota_bytes {
        active.quota_bytes = Set(v);
    }
    let result = policy_repo::update_user_policy(&state.db, active).await?;

    // invalidate cache
    state
        .cache
        .delete(&format!("user_default_policy:{user_id}"))
        .await;

    Ok(result)
}

pub async fn remove_user_policy(state: &AppState, id: i64) -> Result<()> {
    let existing = policy_repo::find_user_policy_by_id(&state.db, id).await?;
    let user_id = existing.user_id;
    let was_default = existing.is_default;

    // 不允许删除用户唯一的策略分配
    let all_policies = policy_repo::find_user_policies(&state.db, user_id).await?;
    if all_policies.len() <= 1 {
        return Err(AsterError::validation_error(
            "cannot remove the only storage policy assigned to this user",
        ));
    }

    policy_repo::delete_user_policy(&state.db, id).await?;

    // 删的是默认策略 → 自动把剩余第一个设为默认
    if was_default {
        let remaining = policy_repo::find_user_policies(&state.db, user_id).await?;
        if let Some(first) = remaining.into_iter().next() {
            let mut active: user_storage_policy::ActiveModel = first.into();
            active.is_default = Set(true);
            policy_repo::update_user_policy(&state.db, active).await?;
        }
    }

    state
        .cache
        .delete(&format!("user_default_policy:{user_id}"))
        .await;

    Ok(())
}

/// 统一的策略缓存失效
async fn invalidate_policy_cache(state: &AppState, policy_id: i64) {
    state.cache.delete(&format!("policy:{policy_id}")).await;
    state.cache.invalidate_prefix("user_default_policy:").await;
}
