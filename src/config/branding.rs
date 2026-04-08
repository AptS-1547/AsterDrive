use crate::config::RuntimeConfig;
use crate::errors::{AsterError, Result};

pub const BRANDING_TITLE_KEY: &str = "branding_title";
pub const BRANDING_DESCRIPTION_KEY: &str = "branding_description";
pub const BRANDING_FAVICON_URL_KEY: &str = "branding_favicon_url";

pub const DEFAULT_BRANDING_TITLE: &str = "AsterDrive";
pub const DEFAULT_BRANDING_DESCRIPTION: &str = "Self-hosted cloud storage";
pub const DEFAULT_BRANDING_FAVICON_URL: &str = "/favicon.svg";

const MAX_BRANDING_TITLE_LEN: usize = 120;
const MAX_BRANDING_DESCRIPTION_LEN: usize = 300;
const MAX_BRANDING_FAVICON_URL_LEN: usize = 2048;

pub fn normalize_title_config_value(value: &str) -> Result<String> {
    normalize_text_value("branding_title", value, MAX_BRANDING_TITLE_LEN)
}

pub fn normalize_description_config_value(value: &str) -> Result<String> {
    normalize_text_value("branding_description", value, MAX_BRANDING_DESCRIPTION_LEN)
}

pub fn normalize_favicon_url_config_value(value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(String::new());
    }
    if normalized.len() > MAX_BRANDING_FAVICON_URL_LEN {
        return Err(AsterError::validation_error(format!(
            "branding_favicon_url exceeds {MAX_BRANDING_FAVICON_URL_LEN} characters",
        )));
    }
    if normalized.chars().any(char::is_whitespace) {
        return Err(AsterError::validation_error(
            "branding_favicon_url cannot contain whitespace",
        ));
    }
    if !is_allowed_favicon_url(normalized) {
        return Err(AsterError::validation_error(
            "branding_favicon_url must be an absolute http(s) URL or a root-relative path",
        ));
    }
    Ok(normalized.to_string())
}

pub fn title_or_default(runtime_config: &RuntimeConfig) -> String {
    string_or_default(
        runtime_config.get(BRANDING_TITLE_KEY),
        DEFAULT_BRANDING_TITLE,
    )
}

pub fn description_or_default(runtime_config: &RuntimeConfig) -> String {
    string_or_default(
        runtime_config.get(BRANDING_DESCRIPTION_KEY),
        DEFAULT_BRANDING_DESCRIPTION,
    )
}

pub fn favicon_url_or_default(runtime_config: &RuntimeConfig) -> String {
    runtime_config
        .get(BRANDING_FAVICON_URL_KEY)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| is_allowed_favicon_url(value))
        .unwrap_or_else(|| DEFAULT_BRANDING_FAVICON_URL.to_string())
}

fn normalize_text_value(field_name: &str, value: &str, max_len: usize) -> Result<String> {
    let normalized = value.trim();
    if normalized.len() > max_len {
        return Err(AsterError::validation_error(format!(
            "{field_name} exceeds {max_len} characters",
        )));
    }
    if normalized.chars().any(char::is_control) {
        return Err(AsterError::validation_error(format!(
            "{field_name} cannot contain control characters",
        )));
    }
    Ok(normalized.to_string())
}

fn string_or_default(value: Option<String>, default: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn is_allowed_favicon_url(value: &str) -> bool {
    value.starts_with('/') || value.starts_with("https://") || value.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::{
        BRANDING_DESCRIPTION_KEY, BRANDING_FAVICON_URL_KEY, BRANDING_TITLE_KEY,
        DEFAULT_BRANDING_DESCRIPTION, DEFAULT_BRANDING_FAVICON_URL, DEFAULT_BRANDING_TITLE,
        description_or_default, favicon_url_or_default, normalize_favicon_url_config_value,
        normalize_title_config_value, title_or_default,
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
    fn title_and_description_trim_and_allow_empty_for_default_reset() {
        assert_eq!(
            normalize_title_config_value("  My Drive  ").unwrap(),
            "My Drive"
        );
        assert_eq!(
            super::normalize_description_config_value("  Private cloud  ").unwrap(),
            "Private cloud"
        );
        assert_eq!(normalize_title_config_value("   ").unwrap(), "");
    }

    #[test]
    fn favicon_url_rejects_whitespace_and_trims() {
        assert_eq!(
            normalize_favicon_url_config_value("  /assets/icon.svg?v=1  ").unwrap(),
            "/assets/icon.svg?v=1"
        );
        assert!(normalize_favicon_url_config_value("https://cdn.example.com/icon 1.svg").is_err());
        assert!(normalize_favicon_url_config_value("javascript:alert(1)").is_err());
        assert!(normalize_favicon_url_config_value("icons/favicon.svg").is_err());
    }

    #[test]
    fn effective_branding_values_fall_back_when_missing_or_blank() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(title_or_default(&runtime_config), DEFAULT_BRANDING_TITLE);
        assert_eq!(
            description_or_default(&runtime_config),
            DEFAULT_BRANDING_DESCRIPTION
        );
        assert_eq!(
            favicon_url_or_default(&runtime_config),
            DEFAULT_BRANDING_FAVICON_URL
        );

        runtime_config.apply(config_model(BRANDING_TITLE_KEY, "  "));
        runtime_config.apply(config_model(BRANDING_DESCRIPTION_KEY, "  "));
        runtime_config.apply(config_model(BRANDING_FAVICON_URL_KEY, " "));

        assert_eq!(title_or_default(&runtime_config), DEFAULT_BRANDING_TITLE);
        assert_eq!(
            description_or_default(&runtime_config),
            DEFAULT_BRANDING_DESCRIPTION
        );
        assert_eq!(
            favicon_url_or_default(&runtime_config),
            DEFAULT_BRANDING_FAVICON_URL
        );

        runtime_config.apply(config_model(
            BRANDING_FAVICON_URL_KEY,
            "javascript:alert(1)",
        ));
        assert_eq!(
            favicon_url_or_default(&runtime_config),
            DEFAULT_BRANDING_FAVICON_URL
        );
    }
}
