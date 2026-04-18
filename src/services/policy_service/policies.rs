//! 存储策略服务子模块：`policies`。

use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};

use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{policy_group_repo, policy_repo};
use crate::entities::storage_policy;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::types::{DriverType, StoredStoragePolicyAllowedTypes, StoredStoragePolicyOptions};

use super::models::{
    CreateStoragePolicyInput, StoragePolicy, StoragePolicyConnectionInput, UpdateStoragePolicyInput,
};
use super::shared::{
    SYSTEM_STORAGE_POLICY_ID, ensure_singleton_group_for_policy, lock_default_group_assignment,
    normalize_connection_fields, serialize_allowed_types, serialize_options,
};

pub async fn list_paginated(
    state: &AppState,
    limit: u64,
    offset: u64,
) -> Result<OffsetPage<StoragePolicy>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) = policy_repo::find_paginated(&state.db, limit, offset).await?;
        Ok((items.into_iter().map(Into::into).collect(), total))
    })
    .await
}

pub async fn get(state: &AppState, id: i64) -> Result<StoragePolicy> {
    policy_repo::find_by_id(&state.db, id).await.map(Into::into)
}

pub async fn create(state: &AppState, input: CreateStoragePolicyInput) -> Result<StoragePolicy> {
    let CreateStoragePolicyInput {
        name,
        connection,
        max_file_size,
        chunk_size,
        is_default,
        allowed_types,
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
    let allowed_types = allowed_types.unwrap_or_default();
    let options = options.unwrap_or_default();

    let txn = crate::db::transaction::begin(&state.db).await?;
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
        allowed_types: Set(serialize_allowed_types(&allowed_types)?),
        options: Set(serialize_options(&options)?),
        is_default: Set(false),
        chunk_size: Set(chunk_size.unwrap_or(5_242_880)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let result = policy_repo::create(&txn, model).await?;
    if is_default {
        lock_default_group_assignment(&txn).await?;
        policy_repo::set_only_default(&txn, result.id).await?;
        let default_group_id = ensure_singleton_group_for_policy(&txn, result.id).await?;
        policy_group_repo::set_only_default_group(&txn, default_group_id).await?;
    }
    crate::db::transaction::commit(txn).await?;
    state.policy_snapshot.reload(&state.db).await?;
    policy_repo::find_by_id(&state.db, result.id)
        .await
        .map(Into::into)
}

pub async fn delete(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;

    if policy.id == SYSTEM_STORAGE_POLICY_ID {
        return Err(AsterError::validation_error(
            "cannot delete the built-in system storage policy",
        ));
    }

    if policy.is_default {
        let all = policy_repo::find_all(&state.db).await?;
        let default_count = all.iter().filter(|p| p.is_default).count();
        if default_count <= 1 {
            return Err(AsterError::validation_error(
                "cannot delete the only default storage policy",
            ));
        }
    }

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

    let cleared =
        crate::db::repository::folder_repo::clear_policy_references(&state.db, id).await?;
    if cleared > 0 {
        tracing::info!("cleared policy_id on {cleared} folders before deleting policy #{id}");
    }

    policy_repo::delete(&state.db, id).await?;

    // 与 update 一致：先 invalidate driver 再 reload snapshot，
    // 避免"策略行已删除但 driver 仍在缓存里"的窗口。
    state.driver_registry.invalidate(id);
    state.policy_snapshot.reload(&state.db).await?;
    Ok(())
}

pub async fn update(
    state: &AppState,
    id: i64,
    input: UpdateStoragePolicyInput,
) -> Result<StoragePolicy> {
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
        allowed_types,
        options,
    } = input;
    let txn = crate::db::transaction::begin(&state.db).await?;
    let existing = policy_repo::find_by_id(&txn, id).await?;
    let existing_endpoint = existing.endpoint.clone();
    let existing_bucket = existing.bucket.clone();
    let final_endpoint = endpoint.unwrap_or_else(|| existing_endpoint.clone());
    let final_bucket = bucket.unwrap_or_else(|| existing_bucket.clone());
    let (normalized_endpoint, normalized_bucket) =
        normalize_connection_fields(existing.driver_type, &final_endpoint, &final_bucket)?;

    if let Some(false) = is_default
        && existing.is_default
        && policy_repo::find_default(&txn).await?.is_some()
    {
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
    if let Some(v) = allowed_types {
        active.allowed_types = Set(serialize_allowed_types(&v)?);
    }
    if let Some(v) = options {
        active.options = Set(serialize_options(&v)?);
    }
    active.updated_at = Set(Utc::now());
    let result = active
        .update(&txn)
        .await
        .map_aster_err(AsterError::database_operation)?;

    if is_default == Some(true) {
        lock_default_group_assignment(&txn).await?;
        policy_repo::set_only_default(&txn, result.id).await?;
        let default_group_id = ensure_singleton_group_for_policy(&txn, result.id).await?;
        policy_group_repo::set_only_default_group(&txn, default_group_id).await?;
    }

    crate::db::transaction::commit(txn).await?;

    // 失效顺序很关键：必须先 invalidate driver 再 reload snapshot。
    // 如果反过来，中间窗口里读请求可能拿到"新 policy model + 旧 driver cache"，
    // 把写操作发到老的 endpoint/bucket/credential 上——无日志、无报错的静默错路由。
    state.driver_registry.invalidate(id);
    state.policy_snapshot.reload(&state.db).await?;

    policy_repo::find_by_id(&state.db, result.id)
        .await
        .map(Into::into)
}

pub async fn test_connection(state: &AppState, id: i64) -> Result<()> {
    let policy = policy_repo::find_by_id(&state.db, id).await?;
    let driver = state.driver_registry.get_driver(&policy)?;

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

pub async fn test_connection_params(input: StoragePolicyConnectionInput) -> Result<()> {
    use crate::storage::drivers::local::LocalDriver;
    use crate::storage::drivers::s3::S3Driver;

    let StoragePolicyConnectionInput {
        driver_type,
        endpoint,
        bucket,
        access_key,
        secret_key,
        base_path,
    } = input;
    let (endpoint, bucket) = normalize_connection_fields(driver_type, &endpoint, &bucket)?;

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
        allowed_types: StoredStoragePolicyAllowedTypes::empty(),
        options: StoredStoragePolicyOptions::empty(),
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
