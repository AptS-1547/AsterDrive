use super::AppState;
use crate::config;
use crate::db;
use crate::errors::Result;
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
        .map_err(|e| crate::errors::AsterError::database_operation(e.to_string()))?;

    // 3. 确保默认存储策略存在
    ensure_default_policy(&database).await?;

    // 4. 驱动注册中心
    let driver_registry = Arc::new(DriverRegistry::new());

    tracing::info!(
        "startup complete — listening on {}:{}",
        cfg.server.host,
        cfg.server.port
    );

    Ok(AppState {
        db: database,
        driver_registry,
        config: cfg,
    })
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
        driver_type: Set("local".to_string()),
        endpoint: Set(String::new()),
        bucket: Set(String::new()),
        access_key: Set(String::new()),
        secret_key: Set(String::new()),
        base_path: Set(data_dir.to_string()),
        max_file_size: Set(0), // 无限制
        allowed_types: Set("[]".to_string()),
        options: Set("{}".to_string()),
        is_default: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    policy_repo::create(db, model).await?;

    tracing::info!("created default local storage policy (data dir: {data_dir})");
    Ok(())
}
