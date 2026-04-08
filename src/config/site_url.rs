use crate::config::RuntimeConfig;
use crate::config::cors;
use crate::errors::Result;

pub const PUBLIC_SITE_URL_KEY: &str = "public_site_url";

pub fn normalize_public_site_url_config_value(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    cors::normalize_origin(trimmed, false)
}

pub fn public_site_url(runtime_config: &RuntimeConfig) -> Option<String> {
    runtime_config
        .get(PUBLIC_SITE_URL_KEY)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn public_app_url(runtime_config: &RuntimeConfig, path: &str) -> Option<String> {
    let base = public_site_url(runtime_config)?;
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    Some(format!("{base}{normalized_path}"))
}

pub fn public_app_url_or_path(runtime_config: &RuntimeConfig, path: &str) -> String {
    public_app_url(runtime_config, path).unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        PUBLIC_SITE_URL_KEY, normalize_public_site_url_config_value, public_app_url,
        public_site_url,
    };
    use crate::config::RuntimeConfig;
    use crate::entities::system_config;
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 1,
            key: key.to_string(),
            value: value.to_string(),
            value_type: "string".to_string(),
            requires_restart: false,
            is_sensitive: false,
            source: "system".to_string(),
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
    fn normalize_public_site_url_rejects_paths_and_non_http_schemes() {
        assert!(normalize_public_site_url_config_value("https://drive.example.com/app").is_err());
        assert!(normalize_public_site_url_config_value("ftp://drive.example.com").is_err());
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
            public_app_url(&runtime_config, "/pv/token/report.docx").as_deref(),
            Some("https://drive.example.com/pv/token/report.docx")
        );
    }
}
