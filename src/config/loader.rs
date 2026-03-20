use super::schema::Config;
use crate::errors::{AsterError, Result};
use config::{Config as RawConfig, Environment, File, FileFormat};
use std::path::Path;

const CONFIG_PATH: &str = "config.toml";

pub fn load() -> Result<Config> {
    // 配置文件不存在时，生成默认配置并写出
    if !Path::new(CONFIG_PATH).exists() {
        create_default_config()?;
    }

    let raw = RawConfig::builder()
        .add_source(File::new("config", FileFormat::Toml).required(false))
        // 环境变量，ASTER__ 前缀，双下划线分隔层级
        .add_source(
            Environment::with_prefix("ASTER")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .map_err(|e| AsterError::config_error(e.to_string()))?;

    let cfg = raw
        .try_deserialize::<Config>()
        .map_err(|e| AsterError::config_error(e.to_string()))?;

    eprintln!("[INFO] Configuration loaded from: {CONFIG_PATH}");
    Ok(cfg)
}

fn create_default_config() -> Result<()> {
    let default = Config::default();
    let toml_str =
        toml::to_string_pretty(&default).map_err(|e| AsterError::config_error(e.to_string()))?;

    let content = format!(
        "# AsterDrive 配置文件\n\
         # 由首次启动自动生成，请根据需要修改\n\
         # 文档: https://asterdrive.docs.esap.cc/config/\n\n\
         {toml_str}"
    );

    std::fs::write(CONFIG_PATH, &content)
        .map_err(|e| AsterError::config_error(format!("failed to write {CONFIG_PATH}: {e}")))?;

    eprintln!("[INFO] Default configuration written to: {CONFIG_PATH}");
    eprintln!("[INFO] Please review and modify it as needed.");
    Ok(())
}
