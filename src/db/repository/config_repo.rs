//! 仓储模块：`config_repo`。

use crate::config::definitions::{ALL_CONFIGS, ConfigDef, MEDIA_PROCESSING_REGISTRY_JSON_KEY};
use crate::config::media_processing;
use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::system_config::{self, Entity as SystemConfig};
use crate::errors::{AsterError, Result};
use crate::types::{MediaProcessorKind, SystemConfigSource, SystemConfigValueType};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TryInsertResult,
};

const BOOTSTRAP_ENABLE_VIPS_CLI_ENV: &str = "ASTER_BOOTSTRAP_ENABLE_VIPS_CLI";
const BOOTSTRAP_ENABLE_FFMPEG_CLI_ENV: &str = "ASTER_BOOTSTRAP_ENABLE_FFMPEG_CLI";
const BOOTSTRAP_MEDIA_PROCESSOR_ENV_FLAGS: &[(MediaProcessorKind, &str)] = &[
    (MediaProcessorKind::VipsCli, BOOTSTRAP_ENABLE_VIPS_CLI_ENV),
    (
        MediaProcessorKind::FfmpegCli,
        BOOTSTRAP_ENABLE_FFMPEG_CLI_ENV,
    ),
];

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

fn resolve_default_value<F>(def: &ConfigDef, get_env: &F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    if def.key == MEDIA_PROCESSING_REGISTRY_JSON_KEY {
        return bootstrap_media_processing_registry_default_value(get_env);
    }

    (def.default_fn)()
}

fn bootstrap_media_processing_registry_default_value<F>(get_env: &F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let enabled_processors = bootstrap_enabled_media_processors(get_env);

    if enabled_processors.is_empty() {
        return media_processing::default_media_processing_registry_json();
    }

    let mut config = media_processing::default_media_processing_registry();
    for processor in &mut config.processors {
        if enabled_processors.contains(&processor.kind) {
            processor.enabled = true;
        }
    }

    serde_json::to_string_pretty(&config)
        .expect("bootstrapped media processing registry should serialize")
}

fn bootstrap_enabled_media_processors<F>(get_env: &F) -> Vec<MediaProcessorKind>
where
    F: Fn(&str) -> Option<String>,
{
    BOOTSTRAP_MEDIA_PROCESSOR_ENV_FLAGS
        .iter()
        .filter_map(|(kind, env_name)| env_flag_enabled(get_env, env_name).then_some(*kind))
        .collect()
}

fn env_flag_enabled<F>(get_env: &F, name: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    get_env(name)
        .as_deref()
        .and_then(parse_bool_like)
        .unwrap_or(false)
}

fn parse_bool_like(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// 确保所有系统配置存在，同步元信息（不覆盖用户修改的 value）
pub async fn ensure_defaults<C: ConnectionTrait>(db: &C) -> Result<usize> {
    ensure_defaults_with_env(db, &|name| std::env::var(name).ok()).await
}

async fn ensure_defaults_with_env<C, F>(db: &C, get_env: &F) -> Result<usize>
where
    C: ConnectionTrait,
    F: Fn(&str) -> Option<String>,
{
    let mut count = 0;

    for def in ALL_CONFIGS {
        let default_value = resolve_default_value(def, get_env);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DatabaseConfig;
    use crate::db;
    use migration::{Migrator, MigratorTrait};

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = db::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            pool_size: 1,
            retry_count: 0,
        })
        .await
        .expect("config repo test DB should connect");
        Migrator::up(&db, None)
            .await
            .expect("config repo migrations should succeed");
        db
    }

    async fn media_processing_registry_config(
        db: &sea_orm::DatabaseConnection,
    ) -> media_processing::MediaProcessingRegistryConfig {
        let stored = find_by_key(db, MEDIA_PROCESSING_REGISTRY_JSON_KEY)
            .await
            .expect("media processing config lookup should succeed")
            .expect("media processing config should exist");
        serde_json::from_str(&stored.value)
            .expect("stored media processing config should be valid JSON")
    }

    #[tokio::test]
    async fn ensure_defaults_keeps_media_processing_registry_default_when_bootstrap_env_disabled() {
        let db = setup_db().await;

        ensure_defaults_with_env(&db, &|_| None)
            .await
            .expect("ensure_defaults should succeed");

        let stored = find_by_key(&db, MEDIA_PROCESSING_REGISTRY_JSON_KEY)
            .await
            .expect("media processing config lookup should succeed")
            .expect("media processing config should exist");

        assert_eq!(
            stored.value,
            media_processing::default_media_processing_registry_json()
        );
    }

    #[tokio::test]
    async fn ensure_defaults_bootstraps_cli_processors_without_losing_default_bindings() {
        let db = setup_db().await;

        ensure_defaults_with_env(&db, &|name| match name {
            BOOTSTRAP_ENABLE_VIPS_CLI_ENV | BOOTSTRAP_ENABLE_FFMPEG_CLI_ENV => {
                Some("1".to_string())
            }
            _ => None,
        })
        .await
        .expect("ensure_defaults should succeed");

        let config = media_processing_registry_config(&db).await;
        let vips =
            media_processing::processor_config_for_kind(&config, MediaProcessorKind::VipsCli)
                .expect("vips config should exist");
        let ffmpeg =
            media_processing::processor_config_for_kind(&config, MediaProcessorKind::FfmpegCli)
                .expect("ffmpeg config should exist");

        assert!(vips.enabled);
        assert_eq!(
            vips.extensions,
            media_processing::default_processor_config_for_kind(MediaProcessorKind::VipsCli)
                .extensions
        );
        assert_eq!(
            vips.config.command.as_deref(),
            Some(media_processing::DEFAULT_VIPS_COMMAND)
        );

        assert!(ffmpeg.enabled);
        assert_eq!(
            ffmpeg.extensions,
            media_processing::default_processor_config_for_kind(MediaProcessorKind::FfmpegCli)
                .extensions
        );
        assert_eq!(
            ffmpeg.config.command.as_deref(),
            Some(media_processing::DEFAULT_FFMPEG_COMMAND)
        );
    }

    #[tokio::test]
    async fn ensure_defaults_does_not_override_existing_media_processing_registry() {
        let db = setup_db().await;
        let existing = r#"{
  "version": 1,
  "processors": [
    {
      "kind": "vips_cli",
      "enabled": false,
      "extensions": [
        "heic"
      ],
      "config": {
        "command": "vips"
      }
    },
    {
      "kind": "ffmpeg_cli",
      "enabled": true,
      "extensions": [
        "mp4"
      ],
      "config": {
        "command": "ffmpeg"
      }
    },
    {
      "kind": "images",
      "enabled": true
    }
  ]
}"#;

        ensure_system_value_if_missing(&db, MEDIA_PROCESSING_REGISTRY_JSON_KEY, existing)
            .await
            .expect("initial media processing config insert should succeed");

        ensure_defaults_with_env(&db, &|name| match name {
            BOOTSTRAP_ENABLE_VIPS_CLI_ENV | BOOTSTRAP_ENABLE_FFMPEG_CLI_ENV => {
                Some("1".to_string())
            }
            _ => None,
        })
        .await
        .expect("ensure_defaults should succeed");

        let stored = find_by_key(&db, MEDIA_PROCESSING_REGISTRY_JSON_KEY)
            .await
            .expect("media processing config lookup should succeed")
            .expect("media processing config should exist");

        assert_eq!(stored.value, existing);
    }
}
