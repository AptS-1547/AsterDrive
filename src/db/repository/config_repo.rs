//! 仓储模块：`config_repo`。

use crate::config::definitions::{ALL_CONFIGS, ConfigDef};
use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::system_config::{self, Entity as SystemConfig};
use crate::errors::{AsterError, Result};
use crate::types::{SystemConfigSource, SystemConfigValueType};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TryInsertResult,
};

fn find_definition(key: &str) -> Option<&'static ConfigDef> {
    ALL_CONFIGS.iter().find(|def| def.key == key)
}

fn build_system_active_model(
    def: &ConfigDef,
    value: String,
    now: chrono::DateTime<Utc>,
    updated_by: Option<i64>,
) -> system_config::ActiveModel {
    system_config::ActiveModel {
        key: Set(def.key.to_string()),
        value: Set(value),
        value_type: Set(def.value_type),
        requires_restart: Set(def.requires_restart),
        is_sensitive: Set(def.is_sensitive),
        source: Set(SystemConfigSource::System),
        namespace: Set(String::new()),
        category: Set(def.category.to_string()),
        description: Set(def.description.to_string()),
        updated_at: Set(now),
        updated_by: Set(updated_by),
        ..Default::default()
    }
}

fn build_custom_active_model(
    key: &str,
    value: String,
    now: chrono::DateTime<Utc>,
    updated_by: Option<i64>,
) -> system_config::ActiveModel {
    system_config::ActiveModel {
        key: Set(key.to_string()),
        value: Set(value),
        value_type: Set(SystemConfigValueType::String),
        requires_restart: Set(false),
        is_sensitive: Set(false),
        source: Set(SystemConfigSource::Custom),
        namespace: Set(String::new()),
        category: Set(String::new()),
        description: Set(String::new()),
        updated_at: Set(now),
        updated_by: Set(updated_by),
        ..Default::default()
    }
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<system_config::Model>> {
    SystemConfig::find()
        .order_by_asc(system_config::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
) -> Result<(Vec<system_config::Model>, u64)> {
    fetch_offset_page(
        db,
        SystemConfig::find().order_by_asc(system_config::Column::Id),
        limit,
        offset,
    )
    .await
}

pub async fn find_by_key<C: ConnectionTrait>(
    db: &C,
    key: &str,
) -> Result<Option<system_config::Model>> {
    SystemConfig::find()
        .filter(system_config::Column::Key.eq(key))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    key: &str,
    value: &str,
    updated_by: i64,
) -> Result<system_config::Model> {
    upsert_with_actor(db, key, value, Some(updated_by)).await
}

pub async fn upsert_with_actor<C: ConnectionTrait>(
    db: &C,
    key: &str,
    value: &str,
    updated_by: Option<i64>,
) -> Result<system_config::Model> {
    let now = Utc::now();
    let active = find_definition(key)
        .map(|def| build_system_active_model(def, value.to_string(), now, updated_by))
        .unwrap_or_else(|| build_custom_active_model(key, value.to_string(), now, updated_by));
    let inserted = match SystemConfig::insert(active)
        .on_conflict_do_nothing_on([system_config::Column::Key])
        .exec(db)
        .await
        .map_err(AsterError::from)?
    {
        TryInsertResult::Inserted(_) => true,
        TryInsertResult::Conflicted => false,
        TryInsertResult::Empty => {
            return Err(AsterError::internal_error(
                "system config upsert produced empty insert result",
            ));
        }
    };

    if !inserted {
        let existing = find_by_key(db, key)
            .await?
            .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))?;
        let mut active: system_config::ActiveModel = existing.into();
        active.value = Set(value.to_string());
        active.updated_at = Set(now);
        active.updated_by = Set(updated_by);
        active.update(db).await.map_err(AsterError::from)?;
    }

    find_by_key(db, key)
        .await?
        .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))
}

pub async fn delete_by_key<C: ConnectionTrait>(db: &C, key: &str) -> Result<()> {
    let existing = find_by_key(db, key)
        .await?
        .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))?;

    // 系统配置不允许删除
    if existing.source == SystemConfigSource::System {
        return Err(AsterError::auth_forbidden(
            "cannot delete system configuration",
        ));
    }

    SystemConfig::delete_by_id(existing.id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

pub async fn ensure_system_value_if_missing<C: ConnectionTrait>(
    db: &C,
    key: &str,
    value: &str,
) -> Result<bool> {
    let def = find_definition(key)
        .ok_or_else(|| AsterError::record_not_found(format!("config key '{key}'")))?;
    let now = Utc::now();
    let inserted =
        match SystemConfig::insert(build_system_active_model(def, value.to_string(), now, None))
            .on_conflict_do_nothing_on([system_config::Column::Key])
            .exec(db)
            .await
            .map_err(AsterError::from)?
        {
            TryInsertResult::Inserted(_) => true,
            TryInsertResult::Conflicted => false,
            TryInsertResult::Empty => {
                return Err(AsterError::internal_error(
                    "ensure_system_value_if_missing produced empty insert result",
                ));
            }
        };

    Ok(inserted)
}

/// 确保所有系统配置存在，同步元信息（不覆盖用户修改的 value）
pub async fn ensure_defaults<C: ConnectionTrait>(db: &C) -> Result<usize> {
    let mut count = 0;

    for def in ALL_CONFIGS {
        let default_value = (def.default_fn)();
        let now = Utc::now();
        let inserted =
            match SystemConfig::insert(build_system_active_model(def, default_value, now, None))
                .on_conflict_do_nothing_on([system_config::Column::Key])
                .exec(db)
                .await
                .map_err(AsterError::from)?
            {
                TryInsertResult::Inserted(_) => true,
                TryInsertResult::Conflicted => false,
                TryInsertResult::Empty => {
                    return Err(AsterError::internal_error(
                        "ensure_defaults produced empty insert result",
                    ));
                }
            };

        if inserted {
            tracing::debug!("initialized config '{}' with default value", def.key);
            count += 1;
            continue;
        }

        let existing = find_by_key(db, def.key)
            .await?
            .ok_or_else(|| AsterError::record_not_found(format!("config key '{}'", def.key)))?;
        let mut active: system_config::ActiveModel = existing.into();
        active.source = Set(SystemConfigSource::System);
        active.value_type = Set(def.value_type);
        active.requires_restart = Set(def.requires_restart);
        active.is_sensitive = Set(def.is_sensitive);
        active.category = Set(def.category.to_string());
        active.description = Set(def.description.to_string());
        active.update(db).await.map_err(AsterError::from)?;
    }

    if count > 0 {
        tracing::info!("initialized {count} default configuration items");
    }

    Ok(count)
}
