//! 配置子模块：`site_url`。

use crate::config::RuntimeConfig;
use crate::config::cors;
use crate::errors::{AsterError, Result};

pub use crate::config::definitions::PUBLIC_SITE_URL_KEY;

pub fn normalize_public_site_url_config_value(value: &str) -> Result<String> {
    Ok(parse_public_site_url_value(value)?.join("\n"))
}

fn public_site_url_entries(value: &str) -> impl Iterator<Item = &str> {
    value
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
}

fn parse_public_site_url_origin(origin: &str) -> Result<String> {
    if origin == "*" {
        return Err(AsterError::validation_error(
            "public_site_url does not support wildcard origins",
        ));
    }

    cors::normalize_origin(origin, false).map_err(|err| {
        AsterError::validation_error(format!(
            "invalid public_site_url origin '{origin}': {}",
            err.message()
        ))
    })
}

pub fn parse_public_site_url_value(value: &str) -> Result<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut origins = Vec::new();
    for origin in public_site_url_entries(trimmed) {
        let normalized = parse_public_site_url_origin(origin)?;
        if !origins.contains(&normalized) {
            origins.push(normalized);
        }
    }

    Ok(origins)
}

pub fn public_site_url_config_value(runtime_config: &RuntimeConfig) -> Option<String> {
    runtime_config
        .get(PUBLIC_SITE_URL_KEY)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn public_site_urls(runtime_config: &RuntimeConfig) -> Vec<String> {
    let Some(value) = public_site_url_config_value(runtime_config) else {
        return Vec::new();
    };

    let mut origins = Vec::new();
    for origin in public_site_url_entries(&value) {
        match parse_public_site_url_origin(origin) {
            Ok(normalized) => {
                if !origins.contains(&normalized) {
                    origins.push(normalized);
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    key = PUBLIC_SITE_URL_KEY,
                    entry = %origin,
                    "invalid runtime public_site_url origin; ignoring entry"
                );
            }
        }
    }

    origins
}

pub fn public_site_url(runtime_config: &RuntimeConfig) -> Option<String> {
    public_site_urls(runtime_config).into_iter().next()
}

pub fn public_site_url_for_request(
    runtime_config: &RuntimeConfig,
    scheme: &str,
    host: &str,
) -> Option<String> {
    let origins = public_site_urls(runtime_config);
    if origins.is_empty() {
        return None;
    }

    let request_origin = cors::normalize_origin(&format!("{scheme}://{host}"), false).ok();
    if let Some(request_origin) = request_origin
        && origins.iter().any(|origin| origin == &request_origin)
    {
        return Some(request_origin);
    }

    origins.into_iter().next()
}

pub fn public_app_url(runtime_config: &RuntimeConfig, path: &str) -> Option<String> {
    let base = public_site_url(runtime_config)?;
    Some(join_origin_and_path(&base, path))
}

pub fn public_app_url_for_request(
    runtime_config: &RuntimeConfig,
    path: &str,
    scheme: &str,
    host: &str,
) -> Option<String> {
    let base = public_site_url_for_request(runtime_config, scheme, host)?;
    Some(join_origin_and_path(&base, path))
}

pub fn public_app_url_or_path(runtime_config: &RuntimeConfig, path: &str) -> String {
    public_app_url(runtime_config, path).unwrap_or_else(|| path.to_string())
}

pub fn public_app_url_or_path_for_request(
    runtime_config: &RuntimeConfig,
    path: &str,
    scheme: &str,
    host: &str,
) -> String {
    public_app_url_for_request(runtime_config, path, scheme, host)
        .unwrap_or_else(|| path.to_string())
}

pub fn join_origin_and_path(base: &str, path: &str) -> String {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    format!("{base}{normalized_path}")
}

#[cfg(test)]
mod tests {
    use super::{
        PUBLIC_SITE_URL_KEY, normalize_public_site_url_config_value, parse_public_site_url_value,
        public_app_url, public_app_url_for_request, public_site_url, public_site_url_for_request,
        public_site_urls,
    };
    use crate::config::RuntimeConfig;
    use crate::entities::system_config;
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 1,
            key: key.to_string(),
            value: value.to_string(),
            value_type: crate::types::SystemConfigValueType::String,
            requires_restart: false,
            is_sensitive: false,
            source: crate::types::SystemConfigSource::System,
            namespace: String::new(),
            category: "general".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn normalize_public_site_url_accepts_empty_and_valid_origins() {
        assert_eq!(normalize_public_site_url_config_value("   ").unwrap(), "");
        assert_eq!(
            normalize_public_site_url_config_value(" HTTPS://Drive.EXAMPLE.com/ ").unwrap(),
            "https://drive.example.com"
        );
        assert_eq!(
            normalize_public_site_url_config_value("http://drive.example.com:8080").unwrap(),
            "http://drive.example.com:8080"
        );
    }

    #[test]
    fn normalize_public_site_url_accepts_ordered_origin_lists() {
        assert_eq!(
            normalize_public_site_url_config_value(
                " HTTPS://Drive.EXAMPLE.com/ , https://Panel.example.com, https://drive.example.com "
            )
            .unwrap(),
            "https://drive.example.com\nhttps://panel.example.com"
        );
        assert_eq!(
            normalize_public_site_url_config_value(
                "https://drive.example.com\nhttps://panel.example.com"
            )
            .unwrap(),
            "https://drive.example.com\nhttps://panel.example.com"
        );
        assert_eq!(
            parse_public_site_url_value("https://drive.example.com,,https://api.example.com")
                .unwrap(),
            vec![
                "https://drive.example.com".to_string(),
                "https://api.example.com".to_string()
            ]
        );
    }

    #[test]
    fn normalize_public_site_url_rejects_paths_and_non_http_schemes() {
        assert!(normalize_public_site_url_config_value("https://drive.example.com/app").is_err());
        assert!(normalize_public_site_url_config_value("ftp://drive.example.com").is_err());
        assert!(normalize_public_site_url_config_value("*.example.com").is_err());
        assert!(normalize_public_site_url_config_value("*").is_err());
    }

    #[test]
    fn public_app_url_joins_configured_origin_with_root_paths() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            PUBLIC_SITE_URL_KEY,
            "https://drive.example.com",
        ));

        assert_eq!(
            public_site_url(&runtime_config).as_deref(),
            Some("https://drive.example.com")
        );
        assert_eq!(
            public_site_urls(&runtime_config),
            vec!["https://drive.example.com".to_string()]
        );
        assert_eq!(
            public_app_url(&runtime_config, "/pv/token/report.docx").as_deref(),
            Some("https://drive.example.com/pv/token/report.docx")
        );
    }

    #[test]
    fn public_site_url_for_request_uses_matching_configured_origin_only() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            PUBLIC_SITE_URL_KEY,
            "https://drive.example.com,https://panel.example.com",
        ));

        assert_eq!(
            public_site_url_for_request(&runtime_config, "https", "panel.example.com").as_deref(),
            Some("https://panel.example.com")
        );
        assert_eq!(
            public_site_url_for_request(&runtime_config, "https", "evil.example.com").as_deref(),
            Some("https://drive.example.com")
        );
        assert_eq!(
            public_app_url_for_request(&runtime_config, "pv/token", "https", "panel.example.com")
                .as_deref(),
            Some("https://panel.example.com/pv/token")
        );
    }

    #[test]
    fn public_site_urls_ignores_invalid_runtime_entries_individually() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            PUBLIC_SITE_URL_KEY,
            "https://drive.example.com\nhttps://panel.example.com/app\nhttps://api.example.com",
        ));

        assert_eq!(
            public_site_urls(&runtime_config),
            vec![
                "https://drive.example.com".to_string(),
                "https://api.example.com".to_string(),
            ]
        );
    }
}
