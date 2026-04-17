use super::tasks::BackgroundTasks;
use sea_orm::DatabaseConnection;
use tokio::signal::unix::{signal, SignalKind};

/// 等待 SIGINT 或 SIGTERM 信号，然后进行优雅关闭
pub async fn wait_for_signal() {
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => tracing::info!("received SIGINT, shutting down gracefully..."),
        _ = sigterm.recv() => tracing::info!("received SIGTERM, shutting down gracefully..."),
    }
}

/// 执行关闭流程：先停止后台任务，再关闭数据库连接并记录日志
pub async fn perform_shutdown(background_tasks: BackgroundTasks, db: DatabaseConnection) {
    tracing::info!("stopping background tasks...");
    background_tasks.shutdown().await;
    tracing::info!("background tasks stopped");

    tracing::info!("closing database connection...");
    if let Err(e) = db.close().await {
        tracing::error!("error closing database connection: {}", e);
    } else {
        tracing::info!("database connection closed");
    }
    tracing::info!("shutdown complete");
}
