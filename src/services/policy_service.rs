use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DbBackend, EntityTrait, QuerySelect, Set, TransactionSession,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{policy_group_repo, policy_repo, user_repo};
use crate::entities::{
    storage_policy, storage_policy_group, storage_policy_group_item, user, user_storage_policy,
};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::storage::s3_config::normalize_s3_endpoint_and_bucket;
use crate::types::DriverType;

const SYSTEM_STORAGE_POLICY_ID: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct StoragePolicySummaryInfo {
    pub id: i64,
    pub name: String,
    pub driver_type: DriverType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct StoragePolicyGroupItemInfo {
    pub id: i64,
    pub policy_id: i64,
    pub priority: i32,
    pub min_file_size: i64,
    pub max_file_size: i64,
    pub policy: StoragePolicySummaryInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct StoragePolicyGroupInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_enabled: bool,
    pub is_default: bool,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub items: Vec<StoragePolicyGroupItemInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct StoragePolicyGroupItemInput {
    pub policy_id: i64,
    pub priority: i32,
    pub min_file_size: i64,
    pub max_file_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PolicyGroupUserMigrationResult {
    pub source_group_id: i64,
    pub target_group_id: i64,
    pub affected_users: u64,
    pub migrated_assignments: u64,
}

#[derive(Debug, Clone)]
pub struct StoragePolicyConnectionInput {
    pub driver_type: DriverType,
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub base_path: String,
}

#[derive(Debug, Clone)]
pub struct CreateStoragePolicyInput {
    pub name: String,
    pub connection: StoragePolicyConnectionInput,
    pub max_file_size: i64,
    pub chunk_size: Option<i64>,
    pub is_default: bool,
    pub options: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateStoragePolicyInput {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub chunk_size: Option<i64>,
    pub is_default: Option<bool>,
    pub options: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateStoragePolicyGroupInput {
    pub name: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub is_default: bool,
    pub items: Vec<StoragePolicyGroupItemInput>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateStoragePolicyGroupInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub items: Option<Vec<StoragePolicyGroupItemInput>>,
}

pub async fn list_all(state: &AppState) -> Result<Vec<storage_policy::Model>> {
    policy_repo::find_all(&state.db).await
}

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<storage_policy::Model>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        policy_repo::find_paginated(&state.db, limit, offset).await
    })
    .await
}

pub async fn get(state: &AppState, id: i64) -> Result<storage_policy::Model> {
    policy_repo::find_by_id(&state.db, id).await
}

pub async fn create(
    state: &AppState,
    input: CreateStoragePolicyInput,
) -> Result<storage_policy::Model> {
    let CreateStoragePolicyInput {
        name,
        connection,
        max_file_size,
        chunk_size,
        is_default,
        options,
    } = input;
    let StoragePolicyConnectionInput {
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
    } = connection;
    let (endpoint, bucket) = normalize_connection_fields(driver_type, &endpoint, &bucket)?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let now = Utc::now();
    let model = storage_policy::ActiveModel {
        name: Set(name),
        driver_type: Set(driver_type),
        endpoint: Set(endpoint),
        bucket: Set(bucket),
        access_key: Set(access_key),
        secret_key: Set(secret_key),
        base_path: Set(base_path),
        max_file_size: Set(max_file_size),
        allowed_types: Set("[]".to_string()),
        options: Set(options.unwrap_or_else(|| "{}".to_string())),
        is_default: Set(false),
        chunk_size: Set(chunk_size.unwrap_or(5_242_880)), // 5MB default
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let result = policy_repo::create(&txn, model).await?;
    if is_default {
        lock_default_policy_assignment(&txn).await?;
        policy_repo::set_only_default(&txn, result.id).await?;
        let default_group_id = ensure_singleton_group_for_policy(&txn, result.id).await?;
        policy_group_repo::set_only_default_group(&txn, default_group_id).await?;
    }
    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    policy_repo::find_by_id(&state.db, result.id).await
}

pub async fn delete(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;

    if policy.id == SYSTEM_STORAGE_POLICY_ID {
        return Err(AsterError::validation_error(
            "cannot delete the built-in system storage policy",
        ));
    }

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

    let group_ref_count = policy_group_repo::count_group_items_by_policy(&state.db, id).await?;
    if group_ref_count > 0 {
        return Err(AsterError::validation_error(format!(
            "cannot delete policy: {group_ref_count} policy group item(s) still reference it"
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

    state.policy_snapshot.reload(&state.db).await?;
    state.driver_registry.invalidate(id);
    Ok(())
}

pub async fn update(
    state: &AppState,
    id: i64,
    input: UpdateStoragePolicyInput,
) -> Result<storage_policy::Model> {
    let UpdateStoragePolicyInput {
        name,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
        max_file_size,
        chunk_size,
        is_default,
        options,
    } = input;
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing = policy_repo::find_by_id(&txn, id).await?;
    let existing_endpoint = existing.endpoint.clone();
    let existing_bucket = existing.bucket.clone();
    let final_endpoint = endpoint.unwrap_or_else(|| existing_endpoint.clone());
    let final_bucket = bucket.unwrap_or_else(|| existing_bucket.clone());
    let (normalized_endpoint, normalized_bucket) =
        normalize_connection_fields(existing.driver_type, &final_endpoint, &final_bucket)?;

    // 不允许取消唯一的系统默认策略
    if let Some(false) = is_default
        && existing.is_default
        && policy_repo::find_default(&txn).await?.is_some()
    {
        // 检查是否是唯一的 default
        let all = policy_repo::find_all(&txn).await?;
        let default_count = all.iter().filter(|p| p.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot unset the only default storage policy",
            ));
        }
    }

    let existing_is_default = existing.is_default;
    let mut active: storage_policy::ActiveModel = existing.into();
    if let Some(v) = name {
        active.name = Set(v);
    }
    if normalized_endpoint != existing_endpoint {
        active.endpoint = Set(normalized_endpoint);
    }
    if normalized_bucket != existing_bucket {
        active.bucket = Set(normalized_bucket);
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
        active.is_default = Set(v && existing_is_default);
    }
    if let Some(v) = options {
        active.options = Set(v);
    }
    active.updated_at = Set(Utc::now());
    let result = active.update(&txn).await.map_err(AsterError::from)?;

    if is_default == Some(true) {
        lock_default_policy_assignment(&txn).await?;
        policy_repo::set_only_default(&txn, result.id).await?;
        let default_group_id = ensure_singleton_group_for_policy(&txn, result.id).await?;
        policy_group_repo::set_only_default_group(&txn, default_group_id).await?;
    }

    txn.commit().await.map_err(AsterError::from)?;

    state.policy_snapshot.reload(&state.db).await?;
    state.driver_registry.invalidate(id);

    policy_repo::find_by_id(&state.db, result.id).await
}

/// 测试存储策略连接是否正常
///
/// - Local: 检查 base_path 目录是否可写
/// - S3: 写入并删除一个测试对象
pub async fn test_connection(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

    // 写一个测试文件然后删除
    let test_path = "_aster_connection_test";
    driver
        .put(test_path, b"ok")
        .await
        .map_aster_err_ctx("write test failed", AsterError::storage_driver_error)?;
    if let Err(e) = driver.delete(test_path).await {
        tracing::warn!("failed to clean up connection test file: {e}");
    }

    Ok(())
}

/// 测试存储策略连接（不保存，用临时构造的 policy）
pub async fn test_connection_params(input: StoragePolicyConnectionInput) -> Result<()> {
    use crate::entities::storage_policy;
    use crate::storage::local::LocalDriver;
    use crate::storage::s3::S3Driver;

    let StoragePolicyConnectionInput {
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
    } = input;
    let (endpoint, bucket) = normalize_connection_fields(driver_type, &endpoint, &bucket)?;

    // 构造一个临时 policy model 用于创建 driver
    let fake_policy = storage_policy::Model {
        id: 0,
        name: String::new(),
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
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
        .map_aster_err_ctx("connection test failed", AsterError::storage_driver_error)?;
    if let Err(e) = driver.delete(test_path).await {
        tracing::warn!("failed to clean up connection test file: {e}");
    }

    Ok(())
}

fn normalize_connection_fields(
    driver_type: DriverType,
    endpoint: &str,
    bucket: &str,
) -> Result<(String, String)> {
    match driver_type {
        DriverType::Local => Ok((endpoint.trim().to_string(), bucket.trim().to_string())),
        DriverType::S3 => {
            let normalized = normalize_s3_endpoint_and_bucket(endpoint, bucket)?;
            Ok((normalized.endpoint, normalized.bucket))
        }
    }
}

fn build_group_info(
    state: &AppState,
    group: &storage_policy_group::Model,
) -> StoragePolicyGroupInfo {
    let items = state
        .policy_snapshot
        .get_policy_group_items(group.id)
        .into_iter()
        .map(|resolved| {
            let policy = resolved.policy;
            StoragePolicyGroupItemInfo {
                id: resolved.item.id,
                policy_id: resolved.item.policy_id,
                priority: resolved.item.priority,
                min_file_size: resolved.item.min_file_size,
                max_file_size: resolved.item.max_file_size,
                policy: StoragePolicySummaryInfo {
                    id: policy.id,
                    name: policy.name,
                    driver_type: policy.driver_type,
                },
            }
        })
        .collect();

    StoragePolicyGroupInfo {
        id: group.id,
        name: group.name.clone(),
        description: group.description.clone(),
        is_enabled: group.is_enabled,
        is_default: group.is_default,
        created_at: group.created_at,
        updated_at: group.updated_at,
        items,
    }
}

fn migrated_group_name(policy_name: &str) -> String {
    format!("Migrated · {policy_name}")
}

fn migrated_group_description(policy_id: i64) -> String {
    format!("Automatically migrated singleton group for storage policy #{policy_id}")
}

async fn find_existing_migrated_group_id<C: sea_orm::ConnectionTrait>(
    db: &C,
    policy: &storage_policy::Model,
) -> Result<Option<i64>> {
    let groups = policy_group_repo::find_all_groups(db).await?;
    let items = policy_group_repo::find_all_group_items(db).await?;
    let mut items_by_group_id =
        std::collections::HashMap::<i64, Vec<storage_policy_group_item::Model>>::new();
    for item in items {
        items_by_group_id
            .entry(item.group_id)
            .or_default()
            .push(item);
    }

    let expected_name = migrated_group_name(&policy.name);
    let expected_description = migrated_group_description(policy.id);
    for group in groups {
        if !group.is_enabled
            || (group.name != expected_name && group.description != expected_description)
        {
            continue;
        }

        let Some(group_items) = items_by_group_id.get(&group.id) else {
            continue;
        };
        if group_items.len() == 1 && group_items[0].policy_id == policy.id {
            return Ok(Some(group.id));
        }
    }

    Ok(None)
}

async fn validate_group_items<C: sea_orm::ConnectionTrait>(
    db: &C,
    items: &[StoragePolicyGroupItemInput],
) -> Result<()> {
    if items.is_empty() {
        return Err(AsterError::validation_error(
            "storage policy group must contain at least one policy",
        ));
    }

    let mut seen_policies = std::collections::HashSet::new();
    let mut seen_priorities = std::collections::HashSet::new();
    for item in items {
        if item.priority <= 0 {
            return Err(AsterError::validation_error(
                "group item priority must be greater than 0",
            ));
        }
        if item.min_file_size < 0 || item.max_file_size < 0 {
            return Err(AsterError::validation_error(
                "file size rules must be non-negative",
            ));
        }
        if item.max_file_size != 0 && item.max_file_size <= item.min_file_size {
            return Err(AsterError::validation_error(
                "max_file_size must be greater than min_file_size",
            ));
        }
        if !seen_policies.insert(item.policy_id) {
            return Err(AsterError::validation_error(
                "duplicate policy_id in storage policy group items",
            ));
        }
        if !seen_priorities.insert(item.priority) {
            return Err(AsterError::validation_error(
                "duplicate priority in storage policy group items",
            ));
        }
        policy_repo::find_by_id(db, item.policy_id).await?;
    }

    Ok(())
}

async fn replace_group_items<C: sea_orm::ConnectionTrait>(
    db: &C,
    group_id: i64,
    items: &[StoragePolicyGroupItemInput],
) -> Result<()> {
    policy_group_repo::delete_group_items_by_group(db, group_id).await?;
    let now = Utc::now();
    for item in items {
        policy_group_repo::create_group_item(
            db,
            storage_policy_group_item::ActiveModel {
                group_id: Set(group_id),
                policy_id: Set(item.policy_id),
                priority: Set(item.priority),
                min_file_size: Set(item.min_file_size),
                max_file_size: Set(item.max_file_size),
                created_at: Set(now),
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

async fn lock_default_group_assignment<C: sea_orm::ConnectionTrait>(db: &C) -> Result<()> {
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => {
            let row = storage_policy::Entity::find_by_id(SYSTEM_STORAGE_POLICY_ID)
                .lock_exclusive()
                .one(db)
                .await
                .map_err(AsterError::from)?;
            if row.is_none() {
                return Err(AsterError::storage_policy_not_found(format!(
                    "policy #{}",
                    SYSTEM_STORAGE_POLICY_ID
                )));
            }
        }
        DbBackend::Sqlite => {
            policy_repo::find_by_id(db, SYSTEM_STORAGE_POLICY_ID).await?;
        }
        _ => {
            policy_repo::find_by_id(db, SYSTEM_STORAGE_POLICY_ID).await?;
        }
    }

    Ok(())
}

async fn lock_default_policy_assignment<C: sea_orm::ConnectionTrait>(db: &C) -> Result<()> {
    lock_default_group_assignment(db).await
}

pub async fn ensure_policy_groups_seeded<C>(db: &C) -> Result<()>
where
    C: sea_orm::ConnectionTrait + TransactionTrait,
{
    let default_policy = match policy_repo::find_default(db).await? {
        Some(policy) => policy,
        None => return Ok(()),
    };

    let txn = db.begin().await.map_err(AsterError::from)?;
    let result = async {
        let default_group = match policy_group_repo::find_default_group(&txn).await? {
            Some(group) => {
                let items = policy_group_repo::find_group_items(&txn, group.id).await?;
                if items.is_empty() {
                    policy_group_repo::create_group_item(
                        &txn,
                        storage_policy_group_item::ActiveModel {
                            group_id: Set(group.id),
                            policy_id: Set(default_policy.id),
                            priority: Set(1),
                            min_file_size: Set(0),
                            max_file_size: Set(0),
                            created_at: Set(Utc::now()),
                            ..Default::default()
                        },
                    )
                    .await?;
                }
                group
            }
            None => {
                let now = Utc::now();
                let group = policy_group_repo::create_group(
                    &txn,
                    storage_policy_group::ActiveModel {
                        name: Set("Default Policy Group".to_string()),
                        description: Set(
                            "System default storage policy group created automatically".to_string(),
                        ),
                        is_enabled: Set(true),
                        is_default: Set(false),
                        created_at: Set(now),
                        updated_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                policy_group_repo::create_group_item(
                    &txn,
                    storage_policy_group_item::ActiveModel {
                        group_id: Set(group.id),
                        policy_id: Set(default_policy.id),
                        priority: Set(1),
                        min_file_size: Set(0),
                        max_file_size: Set(0),
                        created_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                group
            }
        };
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, default_group.id).await?;

        let users_without_group = user_repo::find_all(&txn).await?;
        let users_without_group = users_without_group
            .into_iter()
            .filter(|user| user.policy_group_id.is_none())
            .collect::<Vec<_>>();
        if users_without_group.is_empty() {
            return Ok(());
        }

        let users_without_group_ids = users_without_group
            .iter()
            .map(|user| user.id)
            .collect::<std::collections::HashSet<_>>();
        let legacy_assignments = user_storage_policy::Entity::find().all(&txn).await?;
        let legacy_assignments = legacy_assignments
            .into_iter()
            .filter(|assignment| users_without_group_ids.contains(&assignment.user_id))
            .collect::<Vec<_>>();

        let mut singleton_group_by_policy_id =
            std::collections::HashMap::from([(default_policy.id, default_group.id)]);

        if !legacy_assignments.is_empty() {
            let policies = policy_repo::find_all(&txn).await?;
            let policy_by_id = policies
                .into_iter()
                .map(|policy| (policy.id, policy))
                .collect::<std::collections::HashMap<_, _>>();

            for assignment in &legacy_assignments {
                if singleton_group_by_policy_id.contains_key(&assignment.policy_id) {
                    continue;
                }
                let Some(policy) = policy_by_id.get(&assignment.policy_id) else {
                    continue;
                };
                if let Some(existing_group_id) =
                    find_existing_migrated_group_id(&txn, policy).await?
                {
                    singleton_group_by_policy_id.insert(policy.id, existing_group_id);
                    continue;
                }

                let now = Utc::now();
                let group = policy_group_repo::create_group(
                    &txn,
                    storage_policy_group::ActiveModel {
                        name: Set(migrated_group_name(&policy.name)),
                        description: Set(migrated_group_description(policy.id)),
                        is_enabled: Set(true),
                        is_default: Set(false),
                        created_at: Set(now),
                        updated_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                policy_group_repo::create_group_item(
                    &txn,
                    storage_policy_group_item::ActiveModel {
                        group_id: Set(group.id),
                        policy_id: Set(policy.id),
                        priority: Set(1),
                        min_file_size: Set(0),
                        max_file_size: Set(0),
                        created_at: Set(now),
                        ..Default::default()
                    },
                )
                .await?;
                singleton_group_by_policy_id.insert(policy.id, group.id);
            }
        }

        let mut selected_assignment_by_user_id = HashMap::new();
        for assignment in legacy_assignments {
            selected_assignment_by_user_id
                .entry(assignment.user_id)
                .and_modify(|current: &mut user_storage_policy::Model| {
                    let should_replace = assignment.is_default && !current.is_default
                        || (assignment.is_default == current.is_default
                            && assignment.id < current.id);
                    if should_replace {
                        *current = assignment.clone();
                    }
                })
                .or_insert(assignment);
        }

        for user_model in users_without_group {
            let group_id = selected_assignment_by_user_id
                .get(&user_model.id)
                .and_then(|assignment| singleton_group_by_policy_id.get(&assignment.policy_id))
                .copied()
                .unwrap_or(default_group.id);
            let mut active: user::ActiveModel = user_model.into();
            active.policy_group_id = Set(Some(group_id));
            active.updated_at = Set(Utc::now());
            active.update(&txn).await.map_err(AsterError::from)?;
        }

        Ok(())
    }
    .await;

    match result {
        Ok(()) => txn.commit().await.map_err(AsterError::from),
        Err(err) => {
            txn.rollback().await.map_err(AsterError::from)?;
            Err(err)
        }
    }
}

pub async fn list_groups_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<StoragePolicyGroupInfo>> {
    let page = load_offset_page(limit, offset, 100, |limit, offset| async move {
        policy_group_repo::find_groups_paginated(&state.db, limit, offset).await
    })
    .await?;
    Ok(OffsetPage {
        items: page
            .items
            .iter()
            .map(|group| build_group_info(state, group))
            .collect(),
        total: page.total,
        limit: page.limit,
        offset: page.offset,
    })
}

pub async fn get_group(state: &AppState, id: i64) -> Result<StoragePolicyGroupInfo> {
    let group = policy_group_repo::find_group_by_id(&state.db, id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn create_group(
    state: &AppState,
    input: CreateStoragePolicyGroupInput,
) -> Result<StoragePolicyGroupInfo> {
    let CreateStoragePolicyGroupInput {
        name,
        description,
        is_enabled,
        is_default,
        items,
    } = input;
    if is_default && !is_enabled {
        return Err(AsterError::validation_error(
            "default storage policy group must be enabled",
        ));
    }

    validate_group_items(&state.db, &items).await?;

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let now = Utc::now();
    let group = policy_group_repo::create_group(
        &txn,
        storage_policy_group::ActiveModel {
            name: Set(name),
            description: Set(description.unwrap_or_default()),
            is_enabled: Set(is_enabled),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    replace_group_items(&txn, group.id, &items).await?;
    if is_default {
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, group.id).await?;
    }
    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    let group = policy_group_repo::find_group_by_id(&state.db, group.id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn update_group(
    state: &AppState,
    id: i64,
    input: UpdateStoragePolicyGroupInput,
) -> Result<StoragePolicyGroupInfo> {
    let UpdateStoragePolicyGroupInput {
        name,
        description,
        is_enabled,
        is_default,
        items,
    } = input;
    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let existing = policy_group_repo::find_group_by_id(&txn, id).await?;
    let next_is_enabled = is_enabled.unwrap_or(existing.is_enabled);
    let next_is_default = is_default.unwrap_or(existing.is_default);

    if let Some(false) = is_enabled {
        if next_is_default {
            return Err(AsterError::validation_error(
                "cannot disable the default storage policy group; set another group as default first",
            ));
        }

        if existing.is_enabled {
            let assignment_count =
                policy_group_repo::count_user_group_assignments(&txn, id).await?;
            if assignment_count > 0 {
                return Err(AsterError::validation_error(format!(
                    "cannot disable policy group: {assignment_count} user assignment(s) still reference it"
                )));
            }
        }
    }

    if let Some(true) = is_default
        && !next_is_enabled
    {
        return Err(AsterError::validation_error(
            "default storage policy group must be enabled",
        ));
    }

    if let Some(false) = is_default
        && existing.is_default
    {
        let all = policy_group_repo::find_all_groups(&txn).await?;
        let default_count = all.iter().filter(|group| group.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot unset the only default storage policy group",
            ));
        }
    }

    if let Some(ref updated_items) = items {
        validate_group_items(&txn, updated_items).await?;
    }

    let mut active: storage_policy_group::ActiveModel = existing.into();
    if let Some(value) = name {
        active.name = Set(value);
    }
    if let Some(value) = description {
        active.description = Set(value);
    }
    if let Some(value) = is_enabled {
        active.is_enabled = Set(value);
    }
    if let Some(value) = is_default {
        active.is_default = Set(value);
    }
    active.updated_at = Set(Utc::now());
    let group = policy_group_repo::update_group(&txn, active).await?;

    if let Some(updated_items) = items {
        replace_group_items(&txn, group.id, &updated_items).await?;
    }

    if is_default == Some(true) {
        lock_default_group_assignment(&txn).await?;
        policy_group_repo::set_only_default_group(&txn, group.id).await?;
    }

    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    let group = policy_group_repo::find_group_by_id(&state.db, group.id).await?;
    Ok(build_group_info(state, &group))
}

pub async fn delete_group(state: &AppState, id: i64) -> Result<()> {
    let group = policy_group_repo::find_group_by_id(&state.db, id).await?;

    if group.is_default {
        let all = policy_group_repo::find_all_groups(&state.db).await?;
        let default_count = all.iter().filter(|item| item.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot delete the only default storage policy group",
            ));
        }
    }

    let assignment_count = policy_group_repo::count_user_group_assignments(&state.db, id).await?;
    if assignment_count > 0 {
        return Err(AsterError::validation_error(format!(
            "cannot delete policy group: {assignment_count} user assignment(s) still reference it"
        )));
    }

    policy_group_repo::delete_group(&state.db, id).await?;
    state.policy_snapshot.reload(&state.db).await?;
    Ok(())
}

pub async fn migrate_group_users(
    state: &AppState,
    source_group_id: i64,
    target_group_id: i64,
) -> Result<PolicyGroupUserMigrationResult> {
    if source_group_id == target_group_id {
        return Err(AsterError::validation_error(
            "source and target storage policy groups must be different",
        ));
    }

    policy_group_repo::find_group_by_id(&state.db, source_group_id).await?;
    let target_group = policy_group_repo::find_group_by_id(&state.db, target_group_id).await?;
    if !target_group.is_enabled {
        return Err(AsterError::validation_error(
            "cannot migrate users to a disabled storage policy group",
        ));
    }
    if policy_group_repo::find_group_items(&state.db, target_group_id)
        .await?
        .is_empty()
    {
        return Err(AsterError::validation_error(
            "cannot migrate users to a storage policy group without policies",
        ));
    }

    let source_users = user_repo::find_by_policy_group(&state.db, source_group_id).await?;
    if source_users.is_empty() {
        return Ok(PolicyGroupUserMigrationResult {
            source_group_id,
            target_group_id,
            affected_users: 0,
            migrated_assignments: 0,
        });
    }

    let txn = state.db.begin().await.map_err(AsterError::from)?;
    let migrated_assignments = source_users.len() as u64;
    for source_user in source_users {
        let mut active: user::ActiveModel = source_user.into();
        active.policy_group_id = Set(Some(target_group_id));
        active.updated_at = Set(Utc::now());
        active.update(&txn).await.map_err(AsterError::from)?;
    }

    txn.commit().await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;

    Ok(PolicyGroupUserMigrationResult {
        source_group_id,
        target_group_id,
        affected_users: migrated_assignments,
        migrated_assignments,
    })
}

async fn ensure_singleton_group_for_policy<C: sea_orm::ConnectionTrait>(
    db: &C,
    policy_id: i64,
) -> Result<i64> {
    let singleton_description = format!(
        "Compatibility singleton group for storage policy #{}",
        policy_id
    );
    let groups = policy_group_repo::find_all_groups(db).await?;
    let items = policy_group_repo::find_all_group_items(db).await?;
    let mut items_by_group_id =
        std::collections::HashMap::<i64, Vec<storage_policy_group_item::Model>>::new();
    for item in items {
        items_by_group_id
            .entry(item.group_id)
            .or_default()
            .push(item);
    }
    for group in groups {
        if group.description != singleton_description || !group.is_enabled {
            continue;
        }
        let Some(group_items) = items_by_group_id.get(&group.id) else {
            continue;
        };
        if group_items.len() == 1 && group_items[0].policy_id == policy_id {
            return Ok(group.id);
        }
    }

    let now = Utc::now();
    let policy = policy_repo::find_by_id(db, policy_id).await?;
    let group = policy_group_repo::create_group(
        db,
        storage_policy_group::ActiveModel {
            name: Set(format!("Singleton · {}", policy.name)),
            description: Set(singleton_description),
            is_enabled: Set(true),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    policy_group_repo::create_group_item(
        db,
        storage_policy_group_item::ActiveModel {
            group_id: Set(group.id),
            policy_id: Set(policy.id),
            priority: Set(1),
            min_file_size: Set(0),
            max_file_size: Set(0),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    Ok(group.id)
}

/// Legacy compatibility shim for callers that still assign a single policy per user.
///
/// Deprecated since 0.1.0 in favor of assigning `users.policy_group_id`
/// and managing storage policy groups directly.
#[deprecated(
    since = "0.1.0",
    note = "legacy single-policy assignment compatibility API; use users.policy_group_id and storage policy groups instead"
)]
pub async fn assign_user_policy(
    state: &AppState,
    user_id: i64,
    policy_id: i64,
    _is_default: bool,
    _quota_bytes: i64,
) -> Result<()> {
    let existing_user = user_repo::find_by_id(&state.db, user_id).await?;
    let group_id = ensure_singleton_group_for_policy(&state.db, policy_id).await?;
    let mut active: user::ActiveModel = existing_user.into();
    active.policy_group_id = Set(Some(group_id));
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await.map_err(AsterError::from)?;
    state.policy_snapshot.reload(&state.db).await?;
    state
        .policy_snapshot
        .set_user_policy_group(updated.id, group_id);
    Ok(())
}
