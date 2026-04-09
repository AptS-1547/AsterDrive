use crate::config::RuntimeConfig;
use crate::errors::{AsterError, Result};

pub const MAIL_SMTP_HOST_KEY: &str = "mail_smtp_host";
pub const MAIL_SMTP_PORT_KEY: &str = "mail_smtp_port";
pub const MAIL_SMTP_USERNAME_KEY: &str = "mail_smtp_username";
pub const MAIL_SMTP_PASSWORD_KEY: &str = "mail_smtp_password";
pub const MAIL_FROM_ADDRESS_KEY: &str = "mail_from_address";
pub const MAIL_FROM_NAME_KEY: &str = "mail_from_name";
pub const MAIL_SECURITY_KEY: &str = "mail_security";

pub const DEFAULT_MAIL_SMTP_PORT: u16 = 587;
pub const DEFAULT_MAIL_SECURITY: bool = true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMailSettings {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_address: String,
    pub from_name: String,
    pub encryption_enabled: bool,
}

impl RuntimeMailSettings {
    pub fn from_runtime_config(runtime_config: &RuntimeConfig) -> Self {
        let smtp_port = runtime_config
            .get(MAIL_SMTP_PORT_KEY)
            .and_then(|raw| parse_port(&raw))
            .unwrap_or(DEFAULT_MAIL_SMTP_PORT);
        let encryption_enabled =
            runtime_config.get_bool_or(MAIL_SECURITY_KEY, DEFAULT_MAIL_SECURITY);

        Self {
            smtp_host: runtime_config.get(MAIL_SMTP_HOST_KEY).unwrap_or_default(),
            smtp_port,
            smtp_username: runtime_config
                .get(MAIL_SMTP_USERNAME_KEY)
                .unwrap_or_default(),
            smtp_password: runtime_config
                .get(MAIL_SMTP_PASSWORD_KEY)
                .unwrap_or_default(),
            from_address: runtime_config
                .get(MAIL_FROM_ADDRESS_KEY)
                .unwrap_or_default(),
            from_name: runtime_config.get(MAIL_FROM_NAME_KEY).unwrap_or_default(),
            encryption_enabled,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.smtp_host.trim().is_empty() && !self.from_address.trim().is_empty()
    }
}

pub fn normalize_smtp_host_config_value(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(String::new());
    }
    if normalized.contains(char::is_whitespace) {
        return Err(AsterError::validation_error(
            "mail_smtp_host cannot contain spaces",
        ));
    }
    Ok(normalized)
}

pub fn normalize_smtp_port_config_value(value: &str) -> Result<String> {
    let Some(port) = parse_port(value) else {
        return Err(AsterError::validation_error(
            "mail_smtp_port must be an integer between 1 and 65535",
        ));
    };
    Ok(port.to_string())
}

pub fn normalize_mail_address_config_value(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(String::new());
    }
    validate_contact_email(&normalized)?;
    Ok(normalized)
}

pub fn normalize_mail_name_config_value(value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.len() > 128 {
        return Err(AsterError::validation_error(
            "mail_from_name must be at most 128 characters",
        ));
    }
    Ok(normalized.to_string())
}

pub fn normalize_mail_security_config_value(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok("true".to_string()),
        "false" | "0" | "no" | "off" => Ok("false".to_string()),
        _ => Err(AsterError::validation_error(
            "mail_security must be 'true' or 'false'",
        )),
    }
}

fn parse_port(value: &str) -> Option<u16> {
    value.trim().parse::<u16>().ok().filter(|port| *port > 0)
}

fn validate_contact_email(email: &str) -> Result<()> {
    if email.len() > 254 {
        return Err(AsterError::validation_error("email is too long"));
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() || !parts[1].contains('.') {
        return Err(AsterError::validation_error("invalid email format"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAIL_SECURITY, DEFAULT_MAIL_SMTP_PORT, MAIL_SECURITY_KEY, MAIL_SMTP_PORT_KEY,
        RuntimeMailSettings, normalize_mail_security_config_value,
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
            category: "mail".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn runtime_mail_settings_use_secure_defaults_when_config_missing() {
        let runtime_config = RuntimeConfig::new();
        let settings = RuntimeMailSettings::from_runtime_config(&runtime_config);

        assert_eq!(settings.smtp_port, DEFAULT_MAIL_SMTP_PORT);
        assert_eq!(settings.encryption_enabled, DEFAULT_MAIL_SECURITY);
    }

    #[test]
    fn runtime_mail_settings_read_boolean_security_values() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(MAIL_SMTP_PORT_KEY, "465"));
        runtime_config.apply(config_model(MAIL_SECURITY_KEY, "false"));

        let settings = RuntimeMailSettings::from_runtime_config(&runtime_config);

        assert_eq!(settings.smtp_port, 465);
        assert!(!settings.encryption_enabled);
    }

    #[test]
    fn normalize_mail_security_config_value_normalizes_boolean_values() {
        assert_eq!(
            normalize_mail_security_config_value(" true ").unwrap(),
            "true"
        );
        assert_eq!(
            normalize_mail_security_config_value("OFF").unwrap(),
            "false"
        );
    }
}
