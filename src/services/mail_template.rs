use serde::{Deserialize, Serialize, de::DeserializeOwned};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::config::{RuntimeConfig, mail, site_url};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::types::MailTemplateCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMail {
    pub subject: String,
    pub text_body: String,
    pub html_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableItem {
    pub token: String,
    pub label_i18n_key: String,
    pub description_i18n_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TemplateVariableGroup {
    pub category: String,
    pub template_code: String,
    pub label_i18n_key: String,
    pub variables: Vec<TemplateVariableItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterActivationPayload {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactChangeConfirmationPayload {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordResetPayload {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordResetNoticePayload {
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactChangeNoticePayload {
    pub username: String,
    pub previous_email: String,
    pub new_email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailTemplatePayload {
    RegisterActivation(RegisterActivationPayload),
    ContactChangeConfirmation(ContactChangeConfirmationPayload),
    PasswordReset(PasswordResetPayload),
    PasswordResetNotice(PasswordResetNoticePayload),
    ContactChangeNotice(ContactChangeNoticePayload),
}

impl MailTemplatePayload {
    pub fn register_activation(username: &str, token: &str) -> Self {
        Self::RegisterActivation(RegisterActivationPayload {
            username: username.to_string(),
            token: token.to_string(),
        })
    }

    pub fn contact_change_confirmation(username: &str, token: &str) -> Self {
        Self::ContactChangeConfirmation(ContactChangeConfirmationPayload {
            username: username.to_string(),
            token: token.to_string(),
        })
    }

    pub fn password_reset(username: &str, token: &str) -> Self {
        Self::PasswordReset(PasswordResetPayload {
            username: username.to_string(),
            token: token.to_string(),
        })
    }

    pub fn password_reset_notice(username: &str) -> Self {
        Self::PasswordResetNotice(PasswordResetNoticePayload {
            username: username.to_string(),
        })
    }

    pub fn contact_change_notice(username: &str, previous_email: &str, new_email: &str) -> Self {
        Self::ContactChangeNotice(ContactChangeNoticePayload {
            username: username.to_string(),
            previous_email: previous_email.to_string(),
            new_email: new_email.to_string(),
        })
    }

    pub fn template_code(&self) -> MailTemplateCode {
        match self {
            Self::RegisterActivation(_) => MailTemplateCode::RegisterActivation,
            Self::ContactChangeConfirmation(_) => MailTemplateCode::ContactChangeConfirmation,
            Self::PasswordReset(_) => MailTemplateCode::PasswordReset,
            Self::PasswordResetNotice(_) => MailTemplateCode::PasswordResetNotice,
            Self::ContactChangeNotice(_) => MailTemplateCode::ContactChangeNotice,
        }
    }

    pub fn serialize_payload(&self) -> Result<String> {
        match self {
            Self::RegisterActivation(payload) => serialize_payload(payload),
            Self::ContactChangeConfirmation(payload) => serialize_payload(payload),
            Self::PasswordReset(payload) => serialize_payload(payload),
            Self::PasswordResetNotice(payload) => serialize_payload(payload),
            Self::ContactChangeNotice(payload) => serialize_payload(payload),
        }
    }
}

pub fn list_template_variable_groups() -> Vec<TemplateVariableGroup> {
    vec![
        template_variable_group(
            MailTemplateCode::RegisterActivation,
            &[
                placeholder_spec(
                    "username",
                    "settings_template_variable_username_label",
                    "settings_template_variable_username_desc",
                ),
                placeholder_spec(
                    "verification_url",
                    "settings_template_variable_verification_url_label",
                    "settings_template_variable_verification_url_desc",
                ),
            ],
        ),
        template_variable_group(
            MailTemplateCode::ContactChangeConfirmation,
            &[
                placeholder_spec(
                    "username",
                    "settings_template_variable_username_label",
                    "settings_template_variable_username_desc",
                ),
                placeholder_spec(
                    "verification_url",
                    "settings_template_variable_verification_url_label",
                    "settings_template_variable_verification_url_desc",
                ),
            ],
        ),
        template_variable_group(
            MailTemplateCode::PasswordReset,
            &[
                placeholder_spec(
                    "username",
                    "settings_template_variable_username_label",
                    "settings_template_variable_username_desc",
                ),
                placeholder_spec(
                    "reset_url",
                    "settings_template_variable_reset_url_label",
                    "settings_template_variable_reset_url_desc",
                ),
            ],
        ),
        template_variable_group(
            MailTemplateCode::PasswordResetNotice,
            &[placeholder_spec(
                "username",
                "settings_template_variable_username_label",
                "settings_template_variable_username_desc",
            )],
        ),
        template_variable_group(
            MailTemplateCode::ContactChangeNotice,
            &[
                placeholder_spec(
                    "username",
                    "settings_template_variable_username_label",
                    "settings_template_variable_username_desc",
                ),
                placeholder_spec(
                    "previous_email",
                    "settings_template_variable_previous_email_label",
                    "settings_template_variable_previous_email_desc",
                ),
                placeholder_spec(
                    "new_email",
                    "settings_template_variable_new_email_label",
                    "settings_template_variable_new_email_desc",
                ),
            ],
        ),
    ]
}

pub fn render(
    runtime_config: &RuntimeConfig,
    template_code: MailTemplateCode,
    payload_json: &str,
) -> Result<RenderedMail> {
    let placeholders = match template_code {
        MailTemplateCode::RegisterActivation => {
            let payload: RegisterActivationPayload =
                deserialize_payload(template_code, payload_json)?;
            let verification_url = verification_link(runtime_config, &payload.token);
            PlaceholderSet {
                text_values: vec![
                    ("username", payload.username.clone()),
                    ("verification_url", verification_url.clone()),
                ],
                html_values: vec![
                    ("username", escape_html(&payload.username)),
                    ("verification_url", escape_html(&verification_url)),
                ],
            }
        }
        MailTemplateCode::ContactChangeConfirmation => {
            let payload: ContactChangeConfirmationPayload =
                deserialize_payload(template_code, payload_json)?;
            let verification_url = verification_link(runtime_config, &payload.token);
            PlaceholderSet {
                text_values: vec![
                    ("username", payload.username.clone()),
                    ("verification_url", verification_url.clone()),
                ],
                html_values: vec![
                    ("username", escape_html(&payload.username)),
                    ("verification_url", escape_html(&verification_url)),
                ],
            }
        }
        MailTemplateCode::PasswordReset => {
            let payload: PasswordResetPayload = deserialize_payload(template_code, payload_json)?;
            let reset_url = password_reset_link(runtime_config, &payload.token);
            PlaceholderSet {
                text_values: vec![
                    ("username", payload.username.clone()),
                    ("reset_url", reset_url.clone()),
                ],
                html_values: vec![
                    ("username", escape_html(&payload.username)),
                    ("reset_url", escape_html(&reset_url)),
                ],
            }
        }
        MailTemplateCode::PasswordResetNotice => {
            let payload: PasswordResetNoticePayload =
                deserialize_payload(template_code, payload_json)?;
            PlaceholderSet {
                text_values: vec![("username", payload.username.clone())],
                html_values: vec![("username", escape_html(&payload.username))],
            }
        }
        MailTemplateCode::ContactChangeNotice => {
            let payload: ContactChangeNoticePayload =
                deserialize_payload(template_code, payload_json)?;
            PlaceholderSet {
                text_values: vec![
                    ("username", payload.username.clone()),
                    ("previous_email", payload.previous_email.clone()),
                    ("new_email", payload.new_email.clone()),
                ],
                html_values: vec![
                    ("username", escape_html(&payload.username)),
                    ("previous_email", escape_html(&payload.previous_email)),
                    ("new_email", escape_html(&payload.new_email)),
                ],
            }
        }
    };

    let subject = render_placeholders(
        mail::template_subject(runtime_config, template_code),
        &placeholders.text_values,
    );
    let html_body = render_placeholders(
        mail::template_html(runtime_config, template_code),
        &placeholders.html_values,
    );
    let text_body = html_to_text(&html_body);

    Ok(RenderedMail {
        subject,
        text_body,
        html_body,
    })
}

fn serialize_payload<T: Serialize>(payload: &T) -> Result<String> {
    serde_json::to_string(payload).map_aster_err_ctx(
        "failed to serialize mail payload",
        AsterError::internal_error,
    )
}

fn deserialize_payload<T: DeserializeOwned>(
    template_code: MailTemplateCode,
    payload_json: &str,
) -> Result<T> {
    serde_json::from_str(payload_json).map_aster_err_ctx(
        &format!("failed to decode {} mail payload", template_code.as_str()),
        AsterError::internal_error,
    )
}

fn verification_link(runtime_config: &RuntimeConfig, token: &str) -> String {
    site_url::public_app_url_or_path(
        runtime_config,
        &format!(
            "/api/v1/auth/contact-verification/confirm?token={}",
            urlencoding::encode(token)
        ),
    )
}

fn password_reset_link(runtime_config: &RuntimeConfig, token: &str) -> String {
    site_url::public_app_url_or_path(
        runtime_config,
        &format!("/reset-password?token={}", urlencoding::encode(token)),
    )
}

struct PlaceholderSpec {
    key: &'static str,
    label_i18n_key: &'static str,
    description_i18n_key: &'static str,
}

const fn placeholder_spec(
    key: &'static str,
    label_i18n_key: &'static str,
    description_i18n_key: &'static str,
) -> PlaceholderSpec {
    PlaceholderSpec {
        key,
        label_i18n_key,
        description_i18n_key,
    }
}

fn template_variable_group(
    template_code: MailTemplateCode,
    variables: &[PlaceholderSpec],
) -> TemplateVariableGroup {
    TemplateVariableGroup {
        category: "mail.template".to_string(),
        template_code: template_code.as_str().to_string(),
        label_i18n_key: format!("settings_mail_template_group_{}", template_code.as_str()),
        variables: variables
            .iter()
            .map(|variable| TemplateVariableItem {
                token: format!("{{{{{}}}}}", variable.key),
                label_i18n_key: variable.label_i18n_key.to_string(),
                description_i18n_key: variable.description_i18n_key.to_string(),
            })
            .collect(),
    }
}

fn render_placeholders(mut template: String, values: &[(&'static str, String)]) -> String {
    for (key, value) in values {
        let placeholder = format!("{{{{{key}}}}}");
        template = template.replace(&placeholder, value);
    }
    template
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn html_to_text(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();
    let mut ignored_tags = Vec::new();

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                if let Some(parsed_tag) = parse_tag(&tag) {
                    if ignored_tags.is_empty() {
                        apply_tag_to_text(&mut output, &parsed_tag);
                    }
                    update_ignored_tags(&mut ignored_tags, &parsed_tag);
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(ch);
            }
            continue;
        }

        if ch == '<' {
            in_tag = true;
            continue;
        }

        if ignored_tags.is_empty() {
            output.push(ch);
        }
    }

    let decoded = decode_html_entities(&output);
    normalize_text_fallback(&decoded)
}

fn apply_tag_to_text(output: &mut String, tag: &ParsedTag) {
    if tag.is_closing {
        return;
    }

    if tag.name == "li" && !output.ends_with("- ") {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("- ");
        return;
    }

    let needs_newline = matches!(
        tag.name.as_str(),
        "p" | "div"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "tr"
            | "table"
            | "br"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    );

    if needs_newline && !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}

fn parse_tag(tag: &str) -> Option<ParsedTag> {
    let trimmed = tag.trim();
    if trimmed.is_empty() || trimmed.starts_with('!') || trimmed.starts_with('?') {
        return None;
    }

    let is_closing = trimmed.starts_with('/');
    let content = if is_closing { &trimmed[1..] } else { trimmed };
    let is_self_closing = content.ends_with('/');
    let name = content
        .trim_end_matches('/')
        .split_whitespace()
        .next()?
        .to_ascii_lowercase();

    Some(ParsedTag {
        name,
        is_closing,
        is_self_closing,
    })
}

fn update_ignored_tags(ignored_tags: &mut Vec<String>, tag: &ParsedTag) {
    if !is_ignored_text_tag(&tag.name) || tag.is_self_closing {
        return;
    }

    if tag.is_closing {
        if ignored_tags.last().is_some_and(|name| name == &tag.name) {
            ignored_tags.pop();
        }
        return;
    }

    ignored_tags.push(tag.name.clone());
}

fn is_ignored_text_tag(name: &str) -> bool {
    matches!(name, "head" | "script" | "style" | "title")
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn normalize_text_fallback(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_blank = true;

    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !last_blank {
                normalized.push('\n');
            }
            last_blank = true;
            continue;
        }

        if !normalized.is_empty() && !normalized.ends_with('\n') {
            normalized.push('\n');
        }
        normalized.push_str(trimmed);
        last_blank = false;
    }

    normalized.trim().to_string()
}

struct PlaceholderSet {
    text_values: Vec<(&'static str, String)>,
    html_values: Vec<(&'static str, String)>,
}

struct ParsedTag {
    name: String,
    is_closing: bool,
    is_self_closing: bool,
}

#[cfg(test)]
mod tests {
    use super::{MailTemplateCode, MailTemplatePayload, render};
    use crate::config::RuntimeConfig;
    use crate::entities::system_config;
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 1,
            key: key.to_string(),
            value: value.to_string(),
            value_type: "multiline".to_string(),
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
    fn render_register_activation_builds_link_and_escapes_html() {
        let runtime_config = RuntimeConfig::new();
        let payload = MailTemplatePayload::register_activation("A&B", "token-123");
        let rendered = render(
            &runtime_config,
            MailTemplateCode::RegisterActivation,
            &payload.serialize_payload().unwrap(),
        )
        .unwrap();

        assert!(rendered.text_body.contains("token=token-123"));
        assert!(rendered.html_body.starts_with("<!doctype html>"));
        assert!(rendered.html_body.contains("A&amp;B"));
    }

    #[test]
    fn html_to_text_generates_multiline_fallback() {
        let html = "<p>Hello &amp; welcome</p><p><a href=\"https://example.com\">https://example.com</a></p>";

        assert_eq!(
            super::html_to_text(html),
            "Hello & welcome\nhttps://example.com"
        );
    }

    #[test]
    fn html_to_text_ignores_head_content() {
        let html = "<!doctype html><html><head><title>Ignore me</title><style>.note { color: red; }</style></head><body><p>Hello</p></body></html>";

        assert_eq!(super::html_to_text(html), "Hello");
    }

    #[test]
    fn render_keeps_existing_full_html_documents() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            crate::config::mail::MAIL_TEMPLATE_PASSWORD_RESET_HTML_KEY,
            "<!doctype html><html><body><p>Hello {{username}}</p></body></html>",
        ));

        let payload = MailTemplatePayload::password_reset("Alice", "token-123");
        let rendered = render(
            &runtime_config,
            MailTemplateCode::PasswordReset,
            &payload.serialize_payload().unwrap(),
        )
        .unwrap();

        assert_eq!(rendered.html_body.matches("<html").count(), 1);
        assert!(rendered.html_body.contains("<p>Hello Alice</p>"));
    }
}
