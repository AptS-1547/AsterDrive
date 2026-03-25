use crate::config::LoggingConfig;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;

pub struct LoggingInitResult {
    pub guard: WorkerGuard,
    pub warning: Option<String>,
}

pub fn init_logging(config: &LoggingConfig) -> LoggingInitResult {
    // 创建 writer：文件（可选轮转）or stdout
    let (writer, warning): (Box<dyn std::io::Write + Send + Sync>, Option<String>) = if !config
        .file
        .is_empty()
    {
        if config.enable_rotation {
            // 按天轮转，保留 max_backups 个历史文件
            let dir = std::path::Path::new(&config.file)
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let filename = std::path::Path::new(&config.file)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("aster_drive.log"));
            let filename_str = filename.to_str().unwrap_or("aster_drive.log");
            match rolling::Builder::new()
                .rotation(rolling::Rotation::DAILY)
                .filename_prefix(filename_str.trim_end_matches(".log"))
                .filename_suffix("log")
                .max_log_files(config.max_backups as usize)
                .build(dir)
            {
                Ok(appender) => (Box::new(appender), None),
                Err(e) => (
                    Box::new(std::io::stdout()),
                    Some(format!(
                        "Failed to create rolling log appender for '{}': {}. Falling back to stdout.",
                        config.file, e
                    )),
                ),
            }
        } else {
            // 不轮转，追加写入单文件
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config.file)
            {
                Ok(file) => (Box::new(file), None),
                Err(e) => (
                    Box::new(std::io::stdout()),
                    Some(format!(
                        "Failed to open log file '{}': {}. Falling back to stdout.",
                        config.file, e
                    )),
                ),
            }
        }
    } else {
        (Box::new(std::io::stdout()), None)
    };

    let (non_blocking_writer, guard) = tracing_appender::non_blocking(writer);

    // 验证 log level
    let mut warning = warning;
    let filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => match tracing_subscriber::EnvFilter::try_new(&config.level) {
            Ok(f) => f,
            Err(e) => {
                let msg = format!(
                    "Invalid logging.level '{}': {}. Falling back to 'info'.",
                    config.level, e
                );
                if let Some(existing) = warning.as_mut() {
                    existing.push(' ');
                    existing.push_str(&msg);
                } else {
                    warning = Some(msg);
                }
                tracing_subscriber::EnvFilter::new("info")
            }
        },
    };

    let is_stdout = config.file.is_empty();

    let builder = tracing_subscriber::fmt()
        .with_writer(non_blocking_writer)
        .with_env_filter(filter)
        .with_level(true)
        .with_ansi(is_stdout);

    if config.format == "json" {
        builder.json().init();
    } else {
        builder.init();
    }

    LoggingInitResult { guard, warning }
}
