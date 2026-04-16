use crate::config::RuntimeConfig;
use crate::errors::{AsterError, Result};

pub const MAIL_OUTBOX_DISPATCH_INTERVAL_SECS_KEY: &str = "mail_outbox_dispatch_interval_secs";
pub const BACKGROUND_TASK_DISPATCH_INTERVAL_SECS_KEY: &str =
    "background_task_dispatch_interval_secs";
pub const BACKGROUND_TASK_MAX_CONCURRENCY_KEY: &str = "background_task_max_concurrency";
pub const BACKGROUND_TASK_MAX_ATTEMPTS_KEY: &str = "background_task_max_attempts";
pub const MAINTENANCE_CLEANUP_INTERVAL_SECS_KEY: &str = "maintenance_cleanup_interval_secs";
pub const BLOB_RECONCILE_INTERVAL_SECS_KEY: &str = "blob_reconcile_interval_secs";
pub const TEAM_MEMBER_LIST_MAX_LIMIT_KEY: &str = "team_member_list_max_limit";
pub const TASK_LIST_MAX_LIMIT_KEY: &str = "task_list_max_limit";
pub const AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY: &str = "avatar_max_upload_size_bytes";
pub const THUMBNAIL_MAX_SOURCE_BYTES_KEY: &str = "thumbnail_max_source_bytes";

pub const DEFAULT_MAIL_OUTBOX_DISPATCH_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_BACKGROUND_TASK_DISPATCH_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY: usize = 1;
pub const DEFAULT_BACKGROUND_TASK_MAX_ATTEMPTS: i32 = 3;
pub const DEFAULT_MAINTENANCE_CLEANUP_INTERVAL_SECS: u64 = 3600;
pub const DEFAULT_BLOB_RECONCILE_INTERVAL_SECS: u64 = 6 * 3600;
pub const DEFAULT_TEAM_MEMBER_LIST_MAX_LIMIT: u64 = 100;
pub const DEFAULT_TASK_LIST_MAX_LIMIT: u64 = 100;
pub const DEFAULT_AVATAR_MAX_UPLOAD_SIZE_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

pub const MAX_LIST_PAGE_LIMIT: u64 = 1000;

pub fn normalize_interval_config_value(key: &str, value: &str) -> Result<String> {
    normalize_positive_u64_config_value(key, value)
}

pub fn normalize_concurrency_config_value(key: &str, value: &str) -> Result<String> {
    normalize_positive_u64_config_value(key, value)
}

pub fn normalize_attempts_config_value(key: &str, value: &str) -> Result<String> {
    normalize_positive_i32_config_value(key, value)
}

pub fn normalize_bytes_config_value(key: &str, value: &str) -> Result<String> {
    normalize_positive_u64_config_value(key, value)
}

pub fn normalize_list_max_limit_config_value(key: &str, value: &str) -> Result<String> {
    let parsed = parse_positive_u64(value).ok_or_else(|| {
        AsterError::validation_error(format!(
            "{key} must be a positive integer between 1 and {MAX_LIST_PAGE_LIMIT}",
        ))
    })?;
    if parsed > MAX_LIST_PAGE_LIMIT {
        return Err(AsterError::validation_error(format!(
            "{key} must be at most {MAX_LIST_PAGE_LIMIT}",
        )));
    }
    Ok(parsed.to_string())
}

pub fn mail_outbox_dispatch_interval_secs(runtime_config: &RuntimeConfig) -> u64 {
    read_positive_u64(
        runtime_config,
        MAIL_OUTBOX_DISPATCH_INTERVAL_SECS_KEY,
        DEFAULT_MAIL_OUTBOX_DISPATCH_INTERVAL_SECS,
    )
}

pub fn background_task_dispatch_interval_secs(runtime_config: &RuntimeConfig) -> u64 {
    read_positive_u64(
        runtime_config,
        BACKGROUND_TASK_DISPATCH_INTERVAL_SECS_KEY,
        DEFAULT_BACKGROUND_TASK_DISPATCH_INTERVAL_SECS,
    )
}

pub fn background_task_max_concurrency(runtime_config: &RuntimeConfig) -> usize {
    usize::try_from(read_positive_u64(
        runtime_config,
        BACKGROUND_TASK_MAX_CONCURRENCY_KEY,
        DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY as u64,
    ))
    .unwrap_or_else(|_| {
        tracing::warn!(
            key = BACKGROUND_TASK_MAX_CONCURRENCY_KEY,
            "background task concurrency config exceeds usize; using default"
        );
        DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY
    })
}

pub fn background_task_max_attempts(runtime_config: &RuntimeConfig) -> i32 {
    read_positive_i32(
        runtime_config,
        BACKGROUND_TASK_MAX_ATTEMPTS_KEY,
        DEFAULT_BACKGROUND_TASK_MAX_ATTEMPTS,
    )
}

pub fn maintenance_cleanup_interval_secs(runtime_config: &RuntimeConfig) -> u64 {
    read_positive_u64(
        runtime_config,
        MAINTENANCE_CLEANUP_INTERVAL_SECS_KEY,
        DEFAULT_MAINTENANCE_CLEANUP_INTERVAL_SECS,
    )
}

pub fn blob_reconcile_interval_secs(runtime_config: &RuntimeConfig) -> u64 {
    read_positive_u64(
        runtime_config,
        BLOB_RECONCILE_INTERVAL_SECS_KEY,
        DEFAULT_BLOB_RECONCILE_INTERVAL_SECS,
    )
}

pub fn team_member_list_max_limit(runtime_config: &RuntimeConfig) -> u64 {
    read_bounded_u64(
        runtime_config,
        TEAM_MEMBER_LIST_MAX_LIMIT_KEY,
        DEFAULT_TEAM_MEMBER_LIST_MAX_LIMIT,
        1,
        MAX_LIST_PAGE_LIMIT,
    )
}

pub fn task_list_max_limit(runtime_config: &RuntimeConfig) -> u64 {
    read_bounded_u64(
        runtime_config,
        TASK_LIST_MAX_LIMIT_KEY,
        DEFAULT_TASK_LIST_MAX_LIMIT,
        1,
        MAX_LIST_PAGE_LIMIT,
    )
}

pub fn avatar_max_upload_size_bytes(runtime_config: &RuntimeConfig) -> usize {
    usize::try_from(read_positive_u64(
        runtime_config,
        AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY,
        DEFAULT_AVATAR_MAX_UPLOAD_SIZE_BYTES,
    ))
    .unwrap_or_else(|_| {
        tracing::warn!(
            key = AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY,
            "avatar upload size config exceeds usize; using default"
        );
        DEFAULT_AVATAR_MAX_UPLOAD_SIZE_BYTES as usize
    })
}

pub fn thumbnail_max_source_bytes(runtime_config: &RuntimeConfig) -> i64 {
    i64::try_from(read_positive_u64(
        runtime_config,
        THUMBNAIL_MAX_SOURCE_BYTES_KEY,
        DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES,
    ))
    .unwrap_or_else(|_| {
        tracing::warn!(
            key = THUMBNAIL_MAX_SOURCE_BYTES_KEY,
            "thumbnail source size config exceeds i64; using default"
        );
        DEFAULT_THUMBNAIL_MAX_SOURCE_BYTES as i64
    })
}

fn normalize_positive_u64_config_value(key: &str, value: &str) -> Result<String> {
    let parsed = parse_positive_u64(value)
        .ok_or_else(|| AsterError::validation_error(format!("{key} must be a positive integer")))?;
    Ok(parsed.to_string())
}

fn normalize_positive_i32_config_value(key: &str, value: &str) -> Result<String> {
    let parsed = parse_positive_i32(value)
        .ok_or_else(|| AsterError::validation_error(format!("{key} must be a positive integer")))?;
    Ok(parsed.to_string())
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    let parsed = value.trim().parse::<u64>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn parse_positive_i32(value: &str) -> Option<i32> {
    let parsed = value.trim().parse::<i32>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn read_positive_u64(runtime_config: &RuntimeConfig, key: &str, default: u64) -> u64 {
    match runtime_config.get(key) {
        Some(raw) => match parse_positive_u64(&raw) {
            Some(value) => value,
            None => {
                tracing::warn!(key, value = %raw, "invalid runtime operations config; using default");
                default
            }
        },
        None => default,
    }
}

fn read_positive_i32(runtime_config: &RuntimeConfig, key: &str, default: i32) -> i32 {
    match runtime_config.get(key) {
        Some(raw) => match parse_positive_i32(&raw) {
            Some(value) => value,
            None => {
                tracing::warn!(key, value = %raw, "invalid runtime operations config; using default");
                default
            }
        },
        None => default,
    }
}

fn read_bounded_u64(
    runtime_config: &RuntimeConfig,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> u64 {
    match runtime_config.get(key) {
        Some(raw) => match raw.trim().parse::<u64>() {
            Ok(value) if (min..=max).contains(&value) => value,
            _ => {
                tracing::warn!(
                    key,
                    value = %raw,
                    min,
                    max,
                    "invalid runtime operations config; using default"
                );
                default
            }
        },
        None => default,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY, BACKGROUND_TASK_MAX_ATTEMPTS_KEY,
        BACKGROUND_TASK_MAX_CONCURRENCY_KEY, BLOB_RECONCILE_INTERVAL_SECS_KEY,
        DEFAULT_AVATAR_MAX_UPLOAD_SIZE_BYTES, DEFAULT_BACKGROUND_TASK_MAX_ATTEMPTS,
        DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY, DEFAULT_BLOB_RECONCILE_INTERVAL_SECS,
        DEFAULT_TASK_LIST_MAX_LIMIT, DEFAULT_TEAM_MEMBER_LIST_MAX_LIMIT, TASK_LIST_MAX_LIMIT_KEY,
        TEAM_MEMBER_LIST_MAX_LIMIT_KEY, avatar_max_upload_size_bytes, background_task_max_attempts,
        background_task_max_concurrency, blob_reconcile_interval_secs,
        normalize_attempts_config_value, normalize_bytes_config_value,
        normalize_concurrency_config_value, normalize_interval_config_value,
        normalize_list_max_limit_config_value, task_list_max_limit, team_member_list_max_limit,
    };
    use crate::config::RuntimeConfig;
    use crate::entities::system_config;
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 1,
            key: key.to_string(),
            value: value.to_string(),
            value_type: crate::types::SystemConfigValueType::Number,
            requires_restart: false,
            is_sensitive: false,
            source: crate::types::SystemConfigSource::System,
            namespace: String::new(),
            category: "operations".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn interval_reader_uses_default_for_missing_and_invalid_values() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            blob_reconcile_interval_secs(&runtime_config),
            DEFAULT_BLOB_RECONCILE_INTERVAL_SECS
        );

        runtime_config.apply(config_model(BLOB_RECONCILE_INTERVAL_SECS_KEY, "0"));
        assert_eq!(
            blob_reconcile_interval_secs(&runtime_config),
            DEFAULT_BLOB_RECONCILE_INTERVAL_SECS
        );
    }

    #[test]
    fn list_limit_reader_accepts_bounded_values_only() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            team_member_list_max_limit(&runtime_config),
            DEFAULT_TEAM_MEMBER_LIST_MAX_LIMIT
        );

        runtime_config.apply(config_model(TEAM_MEMBER_LIST_MAX_LIMIT_KEY, "250"));
        runtime_config.apply(config_model(TASK_LIST_MAX_LIMIT_KEY, "0"));

        assert_eq!(team_member_list_max_limit(&runtime_config), 250);
        assert_eq!(
            task_list_max_limit(&runtime_config),
            DEFAULT_TASK_LIST_MAX_LIMIT
        );
    }

    #[test]
    fn background_task_concurrency_reader_uses_runtime_value_and_default() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            background_task_max_concurrency(&runtime_config),
            DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY
        );

        runtime_config.apply(config_model(BACKGROUND_TASK_MAX_CONCURRENCY_KEY, "3"));
        assert_eq!(background_task_max_concurrency(&runtime_config), 3usize);

        runtime_config.apply(config_model(BACKGROUND_TASK_MAX_CONCURRENCY_KEY, "0"));
        assert_eq!(
            background_task_max_concurrency(&runtime_config),
            DEFAULT_BACKGROUND_TASK_MAX_CONCURRENCY
        );
    }

    #[test]
    fn background_task_attempts_reader_uses_runtime_value_and_default() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            background_task_max_attempts(&runtime_config),
            DEFAULT_BACKGROUND_TASK_MAX_ATTEMPTS
        );

        runtime_config.apply(config_model(BACKGROUND_TASK_MAX_ATTEMPTS_KEY, "5"));
        assert_eq!(background_task_max_attempts(&runtime_config), 5);

        runtime_config.apply(config_model(BACKGROUND_TASK_MAX_ATTEMPTS_KEY, "0"));
        assert_eq!(
            background_task_max_attempts(&runtime_config),
            DEFAULT_BACKGROUND_TASK_MAX_ATTEMPTS
        );
    }

    #[test]
    fn avatar_upload_reader_uses_runtime_value() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY, "4096"));
        assert_eq!(avatar_max_upload_size_bytes(&runtime_config), 4096usize);
    }

    #[test]
    fn normalize_helpers_reject_invalid_values() {
        assert_eq!(
            normalize_interval_config_value("test_interval", " 60 ").unwrap(),
            "60"
        );
        assert_eq!(
            normalize_concurrency_config_value("test_concurrency", "4").unwrap(),
            "4"
        );
        assert_eq!(
            normalize_attempts_config_value("test_attempts", "3").unwrap(),
            "3"
        );
        assert_eq!(
            normalize_bytes_config_value("test_bytes", "1024").unwrap(),
            "1024"
        );
        assert_eq!(
            normalize_list_max_limit_config_value("test_limit", "1000").unwrap(),
            "1000"
        );
        assert!(normalize_interval_config_value("test_interval", "0").is_err());
        assert!(normalize_concurrency_config_value("test_concurrency", "0").is_err());
        assert!(normalize_attempts_config_value("test_attempts", "0").is_err());
        assert!(normalize_bytes_config_value("test_bytes", "-1").is_err());
        assert!(normalize_list_max_limit_config_value("test_limit", "1001").is_err());
    }

    #[test]
    fn avatar_upload_reader_falls_back_for_invalid_values() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(AVATAR_MAX_UPLOAD_SIZE_BYTES_KEY, "invalid"));
        assert_eq!(
            avatar_max_upload_size_bytes(&runtime_config),
            DEFAULT_AVATAR_MAX_UPLOAD_SIZE_BYTES as usize
        );
    }
}
