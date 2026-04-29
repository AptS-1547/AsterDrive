use crate::config;
use crate::config::auth_runtime::AUTH_COOKIE_SECURE_KEY;
use crate::config::node_mode::NodeRuntimeMode;
use crate::db;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::DriverRegistry;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

pub(super) struct CommonRuntimeParts {
    pub cfg: Arc<crate::config::Config>,
    pub database: sea_orm::DatabaseConnection,
    pub driver_registry: Arc<DriverRegistry>,
    pub policy_snapshot: Arc<crate::storage::PolicySnapshot>,
    pub cache: Arc<dyn crate::cache::CacheBackend>,
}

const OBSOLETE_NODE_RUNTIME_MODE_KEY: &str = "node_runtime_mode";
const OBSOLETE_THUMBNAIL_DEFAULT_PROCESSOR_KEY: &str = "thumbnail_default_processor";
const OBSOLETE_THUMBNAIL_VIPS_CLI_ENABLED_KEY: &str = "thumbnail_vips_cli_enabled";
const OBSOLETE_THUMBNAIL_VIPS_COMMAND_KEY: &str = "thumbnail_vips_command";

pub(super) async fn prepare_common(mode: NodeRuntimeMode) -> Result<CommonRuntimeParts> {
    let cfg = config::get_config();

    let database = db::connect(&cfg.database).await?;
    initialize_database_state(&database, cfg.as_ref(), mode).await?;

    let policy_snapshot = Arc::new(crate::storage::PolicySnapshot::new());
    policy_snapshot.reload(&database).await?;

    let driver_registry = Arc::new(DriverRegistry::new());
    match mode {
        NodeRuntimeMode::Primary => driver_registry.reload_primary_state(&database).await?,
        NodeRuntimeMode::Follower => driver_registry.reload_follower_state(&database).await?,
    }

    let cache = crate::cache::create_cache(&cfg.cache).await;

    Ok(CommonRuntimeParts {
        cfg,
        database,
        driver_registry,
        policy_snapshot,
        cache,
    })
}

pub async fn initialize_database_state(
    database: &sea_orm::DatabaseConnection,
    cfg: &crate::config::Config,
    mode: NodeRuntimeMode,
) -> Result<()> {
    Migrator::up(database, None)
        .await
        .map_aster_err(AsterError::database_operation)?;

    if let Some(sqlite_search) = db::sqlite_search::ensure_sqlite_search_ready(database).await? {
        tracing::info!(
            sqlite_version = %sqlite_search.sqlite_version,
            "SQLite search acceleration ready"
        );
    }

    ensure_default_policy(database).await?;
    if matches!(mode, NodeRuntimeMode::Primary) {
        crate::services::policy_service::ensure_policy_groups_seeded(database).await?;
    }

    let bootstrap_cookie_secure = (!cfg.auth.bootstrap_insecure_cookies).to_string();
    crate::db::repository::config_repo::ensure_system_value_if_missing(
        database,
        AUTH_COOKIE_SECURE_KEY,
        &bootstrap_cookie_secure,
    )
    .await?;
    crate::db::repository::config_repo::ensure_defaults(database).await?;
    if matches!(mode, NodeRuntimeMode::Follower) {
        handle_optional_follower_bootstrap(
            crate::services::node_enrollment_service::bootstrap_from_env_if_configured(database)
                .await,
        );
    }
    purge_obsolete_node_runtime_mode(database).await?;
    purge_obsolete_config_key(database, OBSOLETE_THUMBNAIL_DEFAULT_PROCESSOR_KEY).await?;
    purge_obsolete_config_key(database, OBSOLETE_THUMBNAIL_VIPS_CLI_ENABLED_KEY).await?;
    purge_obsolete_config_key(database, OBSOLETE_THUMBNAIL_VIPS_COMMAND_KEY).await?;
    Ok(())
}

fn handle_optional_follower_bootstrap<T>(result: Result<T>) {
    if let Err(error) = result {
        tracing::warn!(
            error = %error,
            master_url_env = crate::services::node_enrollment_service::BOOTSTRAP_REMOTE_MASTER_URL_ENV,
            token_env = crate::services::node_enrollment_service::BOOTSTRAP_REMOTE_ENROLLMENT_TOKEN_ENV,
            "follower enrollment bootstrap from environment failed; continuing startup without applying bootstrap env"
        );
    }
}

async fn purge_obsolete_node_runtime_mode(database: &sea_orm::DatabaseConnection) -> Result<()> {
    purge_obsolete_config_key(database, OBSOLETE_NODE_RUNTIME_MODE_KEY).await
}

async fn purge_obsolete_config_key(
    database: &sea_orm::DatabaseConnection,
    key: &str,
) -> Result<()> {
    let deleted = crate::entities::system_config::Entity::delete_many()
        .filter(crate::entities::system_config::Column::Key.eq(key))
        .exec(database)
        .await
        .map_aster_err(AsterError::database_operation)?
        .rows_affected;

    if deleted > 0 {
        tracing::info!(key, deleted, "removed obsolete runtime config key");
    }

    Ok(())
}

async fn ensure_default_policy(db: &sea_orm::DatabaseConnection) -> Result<()> {
    use crate::db::repository::policy_repo;

    if policy_repo::find_default(db).await?.is_some() {
        return Ok(());
    }

    let all = policy_repo::find_all(db).await?;
    if !all.is_empty() {
        return Ok(());
    }

    let data_dir = "data/uploads";
    std::fs::create_dir_all(data_dir).map_aster_err(|e| {
        AsterError::storage_driver_error(format!("failed to create data dir '{}': {e}", data_dir))
    })?;

    use chrono::Utc;
    use sea_orm::Set;
    let now = Utc::now();
    let model = crate::entities::storage_policy::ActiveModel {
        name: Set("Local Default".to_string()),
        driver_type: Set(crate::types::DriverType::Local),
        endpoint: Set(String::new()),
        bucket: Set(String::new()),
        access_key: Set(String::new()),
        secret_key: Set(String::new()),
        base_path: Set(data_dir.to_string()),
        max_file_size: Set(0),
        allowed_types: Set(crate::types::StoredStoragePolicyAllowedTypes::empty()),
        options: Set(crate::types::StoredStoragePolicyOptions::empty()),
        is_default: Set(true),
        chunk_size: Set(5_242_880),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    policy_repo::create(db, model).await?;

    tracing::info!("created default local storage policy (data dir: {data_dir})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_follower_bootstrap_success_keeps_startup_flow() {
        handle_optional_follower_bootstrap::<()>(Ok(()));
    }

    #[test]
    fn optional_follower_bootstrap_error_does_not_abort_startup() {
        handle_optional_follower_bootstrap::<()>(Err(AsterError::validation_error(
            "enrollment token has already been completed",
        )));
    }
}
