use actix_web::{App, HttpServer, web};

#[cfg(debug_assertions)]
#[global_allocator]
static GLOBAL: aster_drive::alloc::TrackingAlloc = aster_drive::alloc::TrackingAlloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 0. 安装自定义 panic hook（最先执行）
    aster_drive::runtime::panic::install_panic_hook();

    dotenvy::dotenv().ok();

    // 1. 加载配置（会自动创建 config.toml）
    aster_drive::config::init_config().expect("failed to load config");
    let cfg = aster_drive::config::get_config();

    // 2. 初始化日志（基于配置）
    let log_result = aster_drive::runtime::logging::init_logging(&cfg.logging);
    let _log_guard = log_result.guard;
    if let Some(warning) = log_result.warning {
        tracing::warn!("{}", warning);
    }

    // 3. 启动剩余服务（DB、迁移、驱动注册）
    let state = aster_drive::runtime::startup::prepare()
        .await
        .expect("startup failed");

    // 4. 初始化 Prometheus 指标（metrics feature）
    #[cfg(feature = "metrics")]
    {
        match aster_drive::metrics::init_metrics() {
            Ok(()) => {
                aster_drive::metrics::spawn_system_metrics_updater();
                tracing::info!("prometheus metrics initialized");
            }
            Err(e) => tracing::warn!("failed to init metrics: {e}"),
        }
    }

    // 清理 WebDAV 临时文件（上次启动的孤儿文件）
    let _ = tokio::fs::remove_dir_all(&cfg.server.temp_dir).await;

    let host = state.config.server.host.clone();
    let port = state.config.server.port;
    let workers = match state.config.server.workers {
        0 => num_cpus::get(),
        n => n,
    };

    let db = state.db.clone();
    let state = web::Data::new(state);

    let value = state.clone();
    let server = HttpServer::new(move || {
        let db = db.clone();
        App::new()
            .wrap(actix_web::middleware::Compress::default())
            .wrap(aster_drive::api::middleware::request_id::RequestIdMiddleware)
            .wrap(aster_drive::api::middleware::cors::configure_cors())
            // payload 限制：chunk 上传最大 10MB，JSON 1MB
            .app_data(actix_web::web::PayloadConfig::new(10 * 1024 * 1024))
            .app_data(actix_web::web::JsonConfig::default().limit(1024 * 1024))
            .app_data(state.clone())
            .configure(move |cfg| aster_drive::api::configure(cfg, &db))
    })
    .keep_alive(std::time::Duration::from_secs(30))
    .client_request_timeout(std::time::Duration::from_millis(5000))
    .client_disconnect_timeout(std::time::Duration::from_millis(1000))
    .bind((host.as_str(), port))?
    .workers(workers)
    .run();

    let server_handle = server.handle();

    // 后台清理任务（panic-safe，自动重启）
    aster_drive::runtime::tasks::spawn_background_tasks(value);

    // 优雅关闭监听
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("received shutdown signal");
        server_handle.stop(true).await;
    });

    server.await?;
    tracing::info!("server stopped");

    Ok(())
}
