use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub webdav: WebDavConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "ServerConfig::default_host")]
    pub host: String,
    #[serde(default = "ServerConfig::default_port")]
    pub port: u16,
    /// 0 = num_cpus
    #[serde(default)]
    pub workers: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: Self::default_host(),
            port: Self::default_port(),
            workers: 0,
        }
    }
}

impl ServerConfig {
    fn default_host() -> String {
        "127.0.0.1".to_string()
    }
    fn default_port() -> u16 {
        3000
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default = "DatabaseConfig::default_url")]
    pub url: String,
    #[serde(default = "DatabaseConfig::default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "DatabaseConfig::default_retry_count")]
    pub retry_count: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: Self::default_url(),
            pool_size: Self::default_pool_size(),
            retry_count: Self::default_retry_count(),
        }
    }
}

impl DatabaseConfig {
    fn default_url() -> String {
        "sqlite://asterdrive.db?mode=rwc".to_string()
    }
    fn default_pool_size() -> u32 {
        10
    }
    fn default_retry_count() -> u32 {
        3
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthConfig {
    #[serde(default = "AuthConfig::default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "AuthConfig::default_access_ttl")]
    pub access_token_ttl_secs: u64,
    #[serde(default = "AuthConfig::default_refresh_ttl")]
    pub refresh_token_ttl_secs: u64,
    /// 为 cookie 设置 Secure 标志（仅 HTTPS 传输）。本地开发需设为 false。
    #[serde(default = "AuthConfig::default_cookie_secure")]
    pub cookie_secure: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: Self::default_jwt_secret(),
            access_token_ttl_secs: Self::default_access_ttl(),
            refresh_token_ttl_secs: Self::default_refresh_ttl(),
            cookie_secure: true,
        }
    }
}

impl AuthConfig {
    fn default_jwt_secret() -> String {
        use rand::RngExt;
        let mut rng = rand::rng();
        let bytes: [u8; 32] = rng.random();
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
    fn default_access_ttl() -> u64 {
        900
    } // 15 min
    fn default_refresh_ttl() -> u64 {
        604800
    } // 7 days
    fn default_cookie_secure() -> bool {
        true
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CacheConfig {
    #[serde(default = "CacheConfig::default_enabled")]
    pub enabled: bool,
    #[serde(default = "CacheConfig::default_backend")]
    pub backend: String, // "memory" | "redis"
    #[serde(default)]
    pub redis_url: String,
    #[serde(default = "CacheConfig::default_ttl")]
    pub default_ttl: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            backend: Self::default_backend(),
            redis_url: String::new(),
            default_ttl: Self::default_ttl(),
        }
    }
}

impl CacheConfig {
    fn default_enabled() -> bool {
        true
    }
    fn default_backend() -> String {
        "memory".to_string()
    }
    fn default_ttl() -> u64 {
        3600
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "LoggingConfig::default_level")]
    pub level: String,
    #[serde(default = "LoggingConfig::default_format")]
    pub format: String, // "text" | "json"
    #[serde(default)]
    pub file: String, // 留空 = stdout only
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Self::default_level(),
            format: Self::default_format(),
            file: String::new(),
        }
    }
}

impl LoggingConfig {
    fn default_level() -> String {
        "info".to_string()
    }
    fn default_format() -> String {
        "text".to_string()
    }
}

/// WebDAV 静态配置（config.toml）
///
/// 运行时配置通过 system_config 表管理：
/// - `webdav_enabled`: 是否启用 (默认 "true")
/// - `webdav_max_upload_size`: 软上传限制字节数 (默认 "1073741824" = 1GB)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebDavConfig {
    /// 路由前缀，改了要重启
    #[serde(default = "WebDavConfig::default_prefix")]
    pub prefix: String,
    /// actix payload 硬上限，改了要重启。运行时软限制从 DB 读。
    #[serde(default = "WebDavConfig::default_payload_limit")]
    pub payload_limit: usize,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            prefix: Self::default_prefix(),
            payload_limit: Self::default_payload_limit(),
        }
    }
}

impl WebDavConfig {
    fn default_prefix() -> String {
        "/webdav".to_string()
    }
    fn default_payload_limit() -> usize {
        10_737_418_240 // 10 GB 硬上限
    }
}
