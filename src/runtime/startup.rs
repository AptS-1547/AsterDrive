//! 运行时子模块：`startup`。

use super::{FollowerAppState, PrimaryAppState};
use crate::config;
use crate::config::auth_runtime::AUTH_COOKIE_SECURE_KEY;
use crate::config::node_mode::NodeRuntimeMode;
use crate::db;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::DriverRegistry;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

pub struct PreparedPrimaryRuntime {
    pub state: PrimaryAppState,
    pub share_download_rollback_worker: crate::services::share_service::ShareDownloadRollbackWorker,
}

pub struct PreparedFollowerRuntime {
    pub state: FollowerAppState,
}

struct CommonRuntimeParts {
    cfg: Arc<crate::config::Config>,
    database: sea_orm::DatabaseConnection,
    driver_registry: Arc<DriverRegistry>,
    policy_snapshot: Arc<crate::storage::PolicySnapshot>,
    cache: Arc<dyn crate::cache::CacheBackend>,
}

const OBSOLETE_NODE_RUNTIME_MODE_KEY: &str = "node_runtime_mode";

/// 准备主节点运行时（配置和日志应在此之前初始化）
pub async fn prepare_primary() -> Result<PreparedPrimaryRuntime> {
    let common = prepare_common(NodeRuntimeMode::Primary).await?;

    let runtime_config = Arc::new(crate::config::RuntimeConfig::new());
    runtime_config.reload(&common.database).await?;
    let mail_sender = crate::services::mail_service::runtime_sender(runtime_config.clone());
    let (storage_change_tx, _) = tokio::sync::broadcast::channel(
        crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
    );
    let rollback_queue_capacity =
        crate::config::operations::share_download_rollback_queue_capacity(&runtime_config);
    let (share_download_rollback, share_download_rollback_worker) =
        crate::services::share_service::build_share_download_rollback_queue(
            common.database.clone(),
            rollback_queue_capacity,
        );

    tracing::info!(
        mode = NodeRuntimeMode::Primary.as_str(),
        "startup complete — listening on {}:{}",
        common.cfg.server.host,
        common.cfg.server.port
    );

    Ok(PreparedPrimaryRuntime {
        state: PrimaryAppState {
            db: common.database,
            driver_registry: common.driver_registry,
            runtime_config,
            policy_snapshot: common.policy_snapshot,
            config: common.cfg,
            cache: common.cache,
            mail_sender,
            storage_change_tx,
            share_download_rollback,
        },
        share_download_rollback_worker,
    })
}

/// 准备从节点运行时（配置和日志应在此之前初始化）
pub async fn prepare_follower() -> Result<PreparedFollowerRuntime> {
    let common = prepare_common(NodeRuntimeMode::Follower).await?;

    tracing::info!(
        mode = NodeRuntimeMode::Follower.as_str(),
        "startup complete — listening on {}:{}",
        common.cfg.server.host,
        common.cfg.server.port
    );

    Ok(PreparedFollowerRuntime {
        state: FollowerAppState {
            db: common.database,
            driver_registry: common.driver_registry,
            policy_snapshot: common.policy_snapshot,
            config: common.cfg,
            cache: common.cache,
        },
    })
}

async fn prepare_common(mode: NodeRuntimeMode) -> Result<CommonRuntimeParts> {
    let cfg = config::get_config();

    // 1. 连接数据库
    let database = db::connect(&cfg.database).await?;

    // 2. 初始化数据库基础状态
    initialize_database_state(&database, cfg.as_ref(), mode).await?;

    // 3. 初始化策略快照
    let policy_snapshot = Arc::new(crate::storage::PolicySnapshot::new());
    policy_snapshot.reload(&database).await?;

    // 4. 驱动注册中心
    let driver_registry = Arc::new(DriverRegistry::new());
    match mode {
        NodeRuntimeMode::Primary => driver_registry.reload_primary_state(&database).await?,
        NodeRuntimeMode::Follower => driver_registry.reload_follower_state(&database).await?,
    }

    // 5. 初始化缓存
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
    purge_obsolete_node_runtime_mode(database).await?;
    Ok(())
}

async fn purge_obsolete_node_runtime_mode(database: &sea_orm::DatabaseConnection) -> Result<()> {
    let deleted = crate::entities::system_config::Entity::delete_many()
        .filter(crate::entities::system_config::Column::Key.eq(OBSOLETE_NODE_RUNTIME_MODE_KEY))
        .exec(database)
        .await
        .map_aster_err(AsterError::database_operation)?
        .rows_affected;

    if deleted > 0 {
        tracing::info!(
            key = OBSOLETE_NODE_RUNTIME_MODE_KEY,
            deleted,
            "removed obsolete runtime config key"
        );
    }

    Ok(())
}

/// 如果没有默认存储策略，自动创建一个本地存储策略
async fn ensure_default_policy(db: &sea_orm::DatabaseConnection) -> Result<()> {
    use crate::db::repository::policy_repo;

    if policy_repo::find_default(db).await?.is_some() {
        return Ok(());
    }

    // 检查是否有任何策略
    let all = policy_repo::find_all(db).await?;
    if !all.is_empty() {
        return Ok(());
    }

    // 创建默认本地存储策略
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
        max_file_size: Set(0), // 无限制
        allowed_types: Set(crate::types::StoredStoragePolicyAllowedTypes::empty()),
        options: Set(crate::types::StoredStoragePolicyOptions::empty()),
        is_default: Set(true),
        chunk_size: Set(5_242_880), // 5MB default
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    policy_repo::create(db, model).await?;

    tracing::info!("created default local storage policy (data dir: {data_dir})");
    Ok(())
}
