//! AsterDrive 服务端与 CLI 启动入口。

use actix_web::{App, HttpServer, web};
#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};

#[cfg(debug_assertions)]
#[global_allocator]
static GLOBAL: aster_drive::alloc::TrackingAlloc = aster_drive::alloc::TrackingAlloc;

#[cfg(feature = "cli")]
#[derive(Debug, Parser)]
#[command(
    name = "aster_drive",
    version,
    about = "AsterDrive server and operations CLI",
    long_about = "AsterDrive server and operations CLI.\n\nRun without a subcommand to start the server, or use 'serve' explicitly. Use 'config' for offline runtime configuration operations.",
    styles = aster_drive::cli::cli_styles()
)]
struct RootCli {
    #[command(subcommand)]
    command: Option<RootCommand>,
}

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Subcommand)]
enum RootCommand {
    /// Start the AsterDrive server
    Serve,
    /// Run offline health checks for database, config, and storage readiness
    Doctor {
        #[arg(long, env = "ASTER_CLI_OUTPUT_FORMAT", default_value = "auto")]
        output_format: aster_drive::cli::OutputFormat,
        #[command(flatten)]
        args: aster_drive::cli::DoctorArgs,
    },
    /// Manage runtime configuration stored in system_config
    Config {
        #[arg(long, env = "ASTER_CLI_DATABASE_URL")]
        database_url: String,
        #[arg(long, env = "ASTER_CLI_OUTPUT_FORMAT", default_value = "auto")]
        output_format: aster_drive::cli::OutputFormat,
        #[command(subcommand)]
        action: aster_drive::cli::ConfigCommand,
    },
    /// Run an offline database backend migration for a maintenance window
    DatabaseMigrate {
        #[arg(long, env = "ASTER_CLI_OUTPUT_FORMAT", default_value = "auto")]
        output_format: aster_drive::cli::DatabaseMigrateOutputFormat,
        #[command(flatten)]
        args: aster_drive::cli::DatabaseMigrateArgs,
    },
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 0. 安装自定义 panic hook（最先执行）
    aster_drive::runtime::panic::install_panic_hook();

    dotenvy::dotenv().ok();

    #[cfg(feature = "cli")]
    {
        let cli = RootCli::parse();
        match cli.command {
            Some(RootCommand::Doctor {
                output_format,
                args,
            }) => {
                let report = aster_drive::cli::execute_doctor_command(&args).await;
                println!(
                    "{}",
                    aster_drive::cli::render_doctor_success(output_format, &report)
                );
                if report.should_exit_nonzero() {
                    std::process::exit(1);
                }
                return Ok(());
            }
            Some(RootCommand::Config {
                database_url,
                output_format,
                action,
            }) => match aster_drive::cli::execute_config_command(&database_url, &action).await {
                Ok(data) => {
                    println!("{}", aster_drive::cli::render_success(output_format, &data));
                    return Ok(());
                }
                Err(error) => {
                    eprintln!("{}", aster_drive::cli::render_error(output_format, &error));
                    std::process::exit(1);
                }
            },
            Some(RootCommand::DatabaseMigrate {
                output_format,
                args,
            }) => match aster_drive::cli::execute_database_migration(&args).await {
                Ok(data) => {
                    println!(
                        "{}",
                        aster_drive::cli::render_database_migration_success(output_format, &data,)
                    );
                    return Ok(());
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        aster_drive::cli::render_database_migration_error(output_format, &error,)
                    );
                    std::process::exit(1);
                }
            },
            Some(RootCommand::Serve) | None => {}
        }
    }

    // 1. 加载配置（会自动创建 data/config.toml）
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

    // 只清理短命 runtime 临时目录：
    // - 不碰 temp_dir 根，避免误删共享目录（例如 /tmp）里的其他内容；
    // - 不碰 temp_dir/tasks，保留后台任务产物给 retention/排障逻辑处理。
    aster_drive::utils::cleanup_runtime_temp_root(&cfg.server.temp_dir).await;

    let host = state.config.server.host.clone();
    let port = state.config.server.port;
    let workers = match state.config.server.workers {
        0 => num_cpus::get(),
        n => n,
    };

    let configure_db = state.db.clone();
    let shutdown_db = state.db.clone();
    let state = web::Data::new(state);

    let value = state.clone();
    let server = HttpServer::new(move || {
        let db = configure_db.clone();
        App::new()
            .wrap(actix_web::middleware::Compress::default())
            .wrap(aster_drive::api::middleware::request_id::RequestIdMiddleware)
            .wrap(aster_drive::api::middleware::cors::RuntimeCors)
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
    let background_tasks = aster_drive::runtime::tasks::spawn_background_tasks(value);

    // 优雅关闭监听
    tokio::spawn(async move {
        aster_drive::runtime::shutdown::wait_for_signal().await;
        server_handle.stop(true).await;
    });

    let server_result = server.await;
    tracing::info!("server stopped");
    aster_drive::runtime::shutdown::perform_shutdown(background_tasks, shutdown_db).await;

    server_result
}
