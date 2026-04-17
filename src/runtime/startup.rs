use super::AppState;
use crate::config;
use crate::config::auth_runtime::AUTH_COOKIE_SECURE_KEY;
use crate::db;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::DriverRegistry;
use migration::{Migrator, MigratorTrait};
use std::sync::Arc;

/// 准备应用上下文（配置和日志应在此之前初始化）
pub async fn prepare() -> Result<AppState> {
    let cfg = config::get_config();

    // 1. 连接数据库
    let database = db::connect(&cfg.database).await?;

    // 2. 运行迁移
    Migrator::up(&database, None)
        .await
        .map_aster_err(AsterError::database_operation)?;

    if let Some(sqlite_search) = db::sqlite_search::ensure_sqlite_search_ready(&database).await? {
        tracing::info!(
            sqlite_version = %sqlite_search.sqlite_version,
            "SQLite search acceleration ready"
        );
    }

    // 3. 确保默认存储策略存在
    ensure_default_policy(&database).await?;
    crate::services::policy_service::ensure_policy_groups_seeded(&database).await?;

    // 4. 首次初始化认证 cookie 策略，避免纯 HTTP 引导时把自己锁在后台外
    let bootstrap_cookie_secure = (!cfg.auth.bootstrap_insecure_cookies).to_string();
    crate::db::repository::config_repo::ensure_system_value_if_missing(
        &database,
        AUTH_COOKIE_SECURE_KEY,
        &bootstrap_cookie_secure,
    )
    .await?;

    // 5. 确保默认运行时配置存在
    crate::db::repository::config_repo::ensure_defaults(&database).await?;

    // 6. 初始化运行时快照
    let runtime_config = Arc::new(crate::config::RuntimeConfig::new());
    runtime_config.reload(&database).await?;

    let policy_snapshot = Arc::new(crate::storage::PolicySnapshot::new());
    policy_snapshot.reload(&database).await?;

    // 7. 驱动注册中心
    let driver_registry = Arc::new(DriverRegistry::new());

    // 8. 初始化缓存
    let cache = crate::cache::create_cache(&cfg.cache).await;
    let mail_sender = crate::services::mail_service::runtime_sender(runtime_config.clone());

    // 9. 文件变更广播（SSE 消费）
    let (storage_change_tx, _) = tokio::sync::broadcast::channel(
        crate::services::storage_change_service::STORAGE_CHANGE_CHANNEL_CAPACITY,
    );

    tracing::info!(
        "startup complete — listening on {}:{}",
        cfg.server.host,
        cfg.server.port
    );

    let state = AppState {
        db: database,
        driver_registry,
        runtime_config,
        policy_snapshot,
        config: cfg,
        cache,
        mail_sender,
        storage_change_tx,
    };

    Ok(state)
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
    std::fs::create_dir_all(data_dir).map_err(|e| {
        crate::errors::AsterError::storage_driver_error(format!(
            "failed to create data dir '{}': {}",
            data_dir, e
        ))
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
