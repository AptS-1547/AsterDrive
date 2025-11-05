use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub jwt: JwtConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub backend: String, // "local" or "s3"
    pub local_path: Option<PathBuf>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_endpoint: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        dotenvy::dotenv().ok();
        
        let config = config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            .set_default("jwt.expiration_hours", 24)?
            .set_default("storage.backend", "local")?
            .set_default("storage.local_path", "./data/uploads")?
            .build()?;
        
        config.try_deserialize()
    }
}
