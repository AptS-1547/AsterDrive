use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Deserializer, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

pub const PREVIEW_APPS_CONFIG_KEY: &str = "frontend_preview_apps_json";

const PREVIEW_APPS_VERSION: i32 = 1;
const BUILTIN_TABLE_PREVIEW_APP_KEY: &str = "builtin.table";
const DEFAULT_TABLE_PREVIEW_DELIMITER: &str = "auto";
const PREVIEW_APP_ICON_AUDIO: &str = "/static/preview-apps/audio.svg";
const PREVIEW_APP_ICON_CODE: &str = "/static/preview-apps/code.svg";
const PREVIEW_APP_ICON_FILE: &str = "/static/preview-apps/file.svg";
const PREVIEW_APP_ICON_GOOGLE_DRIVE: &str = "/static/preview-apps/google-drive.svg";
const PREVIEW_APP_ICON_IMAGE: &str = "/static/preview-apps/image.svg";
const PREVIEW_APP_ICON_JSON: &str = "/static/preview-apps/json.svg";
const PREVIEW_APP_ICON_MARKDOWN: &str = "/static/preview-apps/markdown.svg";
const PREVIEW_APP_ICON_MICROSOFT_ONEDRIVE: &str = "/static/preview-apps/microsoft-onedrive.svg";
const PREVIEW_APP_ICON_PDF: &str = "/static/preview-apps/pdf.svg";
const PREVIEW_APP_ICON_TABLE: &str = "/static/preview-apps/table.svg";
const PREVIEW_APP_ICON_VIDEO: &str = "/static/preview-apps/video.svg";
const PREVIEW_APP_ICON_XML: &str = "/static/preview-apps/xml.svg";

const REQUIRED_BUILTIN_PREVIEW_APP_KEYS: &[&str] = &[
    "builtin.image",
    "builtin.video",
    "builtin.audio",
    "builtin.pdf",
    "builtin.markdown",
    BUILTIN_TABLE_PREVIEW_APP_KEY,
    "builtin.formatted_json",
    "builtin.formatted_xml",
    "builtin.code",
    "builtin.try_text",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicPreviewAppsConfig {
    #[serde(default = "default_preview_apps_version")]
    pub version: i32,
    #[serde(default)]
    pub apps: Vec<PublicPreviewAppDefinition>,
    #[serde(default)]
    pub rules: Vec<PublicPreviewAppRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicPreviewAppDefinition {
    pub key: String,
    pub provider: PreviewAppProvider,
    pub icon: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_i18n_key: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "PublicPreviewAppConfig::is_empty")]
    pub config: PublicPreviewAppConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicPreviewAppRule {
    #[serde(default)]
    pub matches: PublicPreviewAppMatch,
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_app: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicPreviewAppMatch {
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub mime_prefixes: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub enum PreviewAppProvider {
    Builtin,
    UrlTemplate,
    Wopi,
}

impl PreviewAppProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::UrlTemplate => "url_template",
            Self::Wopi => "wopi",
        }
    }
}

impl<'de> Deserialize<'de> for PreviewAppProvider {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "builtin" => Ok(Self::Builtin),
            "url_template" => Ok(Self::UrlTemplate),
            "wopi" => Ok(Self::Wopi),
            other => Err(serde::de::Error::custom(format!(
                "unsupported preview app provider '{other}'",
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub enum PreviewOpenMode {
    Iframe,
    NewTab,
}

impl<'de> Deserialize<'de> for PreviewOpenMode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "iframe" => Ok(Self::Iframe),
            "new_tab" => Ok(Self::NewTab),
            other => Err(serde::de::Error::custom(format!(
                "unsupported preview open mode '{other}'",
            ))),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicPreviewAppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PreviewOpenMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_template: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_origins: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_url_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub form_fields: BTreeMap<String, String>,
}

impl PublicPreviewAppConfig {
    fn is_empty(&self) -> bool {
        self.delimiter.is_none()
            && self.mode.is_none()
            && self.url_template.is_none()
            && self.allowed_origins.is_empty()
            && self.action.is_none()
            && self.action_url.is_none()
            && self.action_url_template.is_none()
            && self.discovery_url.is_none()
            && self.form_fields.is_empty()
    }
}

pub fn default_public_preview_apps() -> PublicPreviewAppsConfig {
    PublicPreviewAppsConfig {
        version: PREVIEW_APPS_VERSION,
        apps: vec![
            builtin_app(
                "builtin.image",
                PREVIEW_APP_ICON_IMAGE,
                labels(("en", "Image preview"), ("zh", "图片预览")),
            ),
            builtin_app(
                "builtin.video",
                PREVIEW_APP_ICON_VIDEO,
                labels(("en", "Video preview"), ("zh", "视频预览")),
            ),
            builtin_app(
                "builtin.audio",
                PREVIEW_APP_ICON_AUDIO,
                labels(("en", "Audio preview"), ("zh", "音频预览")),
            ),
            builtin_app(
                "builtin.pdf",
                PREVIEW_APP_ICON_PDF,
                labels(("en", "PDF preview"), ("zh", "PDF 预览")),
            ),
            url_template_app(
                "builtin.office_google",
                PREVIEW_APP_ICON_GOOGLE_DRIVE,
                labels(("en", "Google Viewer"), ("zh", "Google 预览器")),
                PublicPreviewAppConfig {
                    mode: Some(PreviewOpenMode::Iframe),
                    url_template: Some(
                        "https://docs.google.com/gview?embedded=true&url={{file_preview_url}}"
                            .to_string(),
                    ),
                    allowed_origins: vec!["https://docs.google.com".to_string()],
                    ..Default::default()
                },
            ),
            url_template_app(
                "builtin.office_microsoft",
                PREVIEW_APP_ICON_MICROSOFT_ONEDRIVE,
                labels(("en", "Microsoft Viewer"), ("zh", "Microsoft 预览器")),
                PublicPreviewAppConfig {
                    mode: Some(PreviewOpenMode::Iframe),
                    url_template: Some(
                        "https://view.officeapps.live.com/op/embed.aspx?src={{file_preview_url}}"
                            .to_string(),
                    ),
                    allowed_origins: vec!["https://view.officeapps.live.com".to_string()],
                    ..Default::default()
                },
            ),
            builtin_app(
                "builtin.markdown",
                PREVIEW_APP_ICON_MARKDOWN,
                labels(("en", "Markdown preview"), ("zh", "Markdown 预览")),
            ),
            builtin_app_with_config(
                BUILTIN_TABLE_PREVIEW_APP_KEY,
                PREVIEW_APP_ICON_TABLE,
                labels(("en", "Table preview"), ("zh", "表格预览")),
                PublicPreviewAppConfig {
                    delimiter: Some(DEFAULT_TABLE_PREVIEW_DELIMITER.to_string()),
                    ..Default::default()
                },
            ),
            builtin_app(
                "builtin.formatted_json",
                PREVIEW_APP_ICON_JSON,
                labels(("en", "Formatted view"), ("zh", "格式化视图")),
            ),
            builtin_app(
                "builtin.formatted_xml",
                PREVIEW_APP_ICON_XML,
                labels(("en", "Formatted view"), ("zh", "格式化视图")),
            ),
            builtin_app(
                "builtin.code",
                PREVIEW_APP_ICON_CODE,
                labels(("en", "Source view"), ("zh", "源码视图")),
            ),
            builtin_app(
                "builtin.try_text",
                PREVIEW_APP_ICON_FILE,
                labels(("en", "Open as text"), ("zh", "以文本方式打开")),
            ),
        ],
        rules: vec![
            category_rule("image", &["builtin.image"], Some("builtin.image")),
            category_rule("video", &["builtin.video"], Some("builtin.video")),
            category_rule("audio", &["builtin.audio"], Some("builtin.audio")),
            category_rule("pdf", &["builtin.pdf"], Some("builtin.pdf")),
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["doc".to_string(), "docx".to_string()],
                    mime_types: vec!["application/msword".to_string()],
                    mime_prefixes: vec![
                        "application/vnd.openxmlformats-officedocument.wordprocessingml"
                            .to_string(),
                    ],
                    ..Default::default()
                },
                apps: vec![
                    "builtin.office_microsoft".to_string(),
                    "builtin.office_google".to_string(),
                ],
                default_app: Some("builtin.office_microsoft".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["xls".to_string(), "xlsx".to_string()],
                    mime_types: vec!["application/vnd.ms-excel".to_string()],
                    mime_prefixes: vec![
                        "application/vnd.openxmlformats-officedocument.spreadsheetml".to_string(),
                    ],
                    ..Default::default()
                },
                apps: vec![
                    "builtin.office_microsoft".to_string(),
                    "builtin.office_google".to_string(),
                ],
                default_app: Some("builtin.office_microsoft".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["ppt".to_string(), "pptx".to_string()],
                    mime_types: vec!["application/vnd.ms-powerpoint".to_string()],
                    mime_prefixes: vec![
                        "application/vnd.openxmlformats-officedocument.presentationml".to_string(),
                    ],
                    ..Default::default()
                },
                apps: vec![
                    "builtin.office_microsoft".to_string(),
                    "builtin.office_google".to_string(),
                ],
                default_app: Some("builtin.office_microsoft".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["odt".to_string()],
                    mime_types: vec!["application/vnd.oasis.opendocument.text".to_string()],
                    ..Default::default()
                },
                apps: vec!["builtin.office_google".to_string()],
                default_app: Some("builtin.office_google".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["ods".to_string()],
                    mime_types: vec!["application/vnd.oasis.opendocument.spreadsheet".to_string()],
                    ..Default::default()
                },
                apps: vec!["builtin.office_google".to_string()],
                default_app: Some("builtin.office_google".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["odp".to_string()],
                    mime_types: vec!["application/vnd.oasis.opendocument.presentation".to_string()],
                    ..Default::default()
                },
                apps: vec!["builtin.office_google".to_string()],
                default_app: Some("builtin.office_google".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["md".to_string(), "markdown".to_string()],
                    mime_types: vec!["text/markdown".to_string()],
                    ..Default::default()
                },
                apps: vec!["builtin.markdown".to_string(), "builtin.code".to_string()],
                default_app: Some("builtin.markdown".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["csv".to_string()],
                    mime_types: vec!["text/csv".to_string()],
                    ..Default::default()
                },
                apps: vec![
                    BUILTIN_TABLE_PREVIEW_APP_KEY.to_string(),
                    "builtin.code".to_string(),
                ],
                default_app: Some(BUILTIN_TABLE_PREVIEW_APP_KEY.to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["tsv".to_string()],
                    mime_types: vec!["text/tab-separated-values".to_string()],
                    ..Default::default()
                },
                apps: vec![
                    BUILTIN_TABLE_PREVIEW_APP_KEY.to_string(),
                    "builtin.code".to_string(),
                ],
                default_app: Some(BUILTIN_TABLE_PREVIEW_APP_KEY.to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["json".to_string()],
                    mime_types: vec!["application/json".to_string()],
                    ..Default::default()
                },
                apps: vec![
                    "builtin.formatted_json".to_string(),
                    "builtin.code".to_string(),
                ],
                default_app: Some("builtin.formatted_json".to_string()),
            },
            PublicPreviewAppRule {
                matches: PublicPreviewAppMatch {
                    extensions: vec!["xml".to_string()],
                    mime_types: vec!["text/xml".to_string(), "application/xml".to_string()],
                    ..Default::default()
                },
                apps: vec![
                    "builtin.formatted_xml".to_string(),
                    "builtin.code".to_string(),
                ],
                default_app: Some("builtin.formatted_xml".to_string()),
            },
            category_rule("text", &["builtin.code"], Some("builtin.code")),
            category_rule("unknown", &["builtin.try_text"], None),
        ],
    }
}

pub fn default_public_preview_apps_json() -> String {
    serde_json::to_string_pretty(&default_public_preview_apps())
        .expect("default preview apps config should serialize")
}

pub fn normalize_public_preview_apps_config_value(value: &str) -> Result<String> {
    let mut config: PublicPreviewAppsConfig = serde_json::from_str(value).map_err(|error| {
        AsterError::validation_error(format!("preview apps config must be valid JSON: {error}"))
    })?;
    validate_preview_apps_config(&mut config)?;
    serde_json::to_string_pretty(&config).map_err(|error| {
        AsterError::internal_error(format!("failed to serialize preview apps config: {error}"))
    })
}

pub fn get_public_preview_apps(state: &AppState) -> PublicPreviewAppsConfig {
    let Some(raw) = state.runtime_config.get(PREVIEW_APPS_CONFIG_KEY) else {
        return default_public_preview_apps();
    };

    match parse_public_preview_apps_config(&raw) {
        Ok(config) => build_public_preview_apps(config),
        Err(error) => {
            tracing::warn!("failed to parse preview apps config: {error}");
            default_public_preview_apps()
        }
    }
}

fn parse_public_preview_apps_config(value: &str) -> Result<PublicPreviewAppsConfig> {
    let mut config: PublicPreviewAppsConfig = serde_json::from_str(value).map_err(|error| {
        AsterError::validation_error(format!("preview apps config must be valid JSON: {error}"))
    })?;
    validate_preview_apps_config(&mut config)?;
    Ok(config)
}

fn build_public_preview_apps(config: PublicPreviewAppsConfig) -> PublicPreviewAppsConfig {
    let enabled_keys: HashSet<&str> = config
        .apps
        .iter()
        .filter(|app| app.enabled)
        .map(|app| app.key.as_str())
        .collect();

    let rules = config
        .rules
        .into_iter()
        .filter_map(|mut rule| {
            rule.apps.retain(|key| enabled_keys.contains(key.as_str()));
            if rule.apps.is_empty() {
                return None;
            }

            if rule
                .default_app
                .as_deref()
                .is_none_or(|key| !rule.apps.iter().any(|candidate| candidate == key))
            {
                rule.default_app = rule.apps.first().cloned();
            }

            Some(rule)
        })
        .collect();

    PublicPreviewAppsConfig {
        version: config.version,
        apps: config.apps.into_iter().filter(|app| app.enabled).collect(),
        rules,
    }
}

fn validate_preview_apps_config(config: &mut PublicPreviewAppsConfig) -> Result<()> {
    if config.version != PREVIEW_APPS_VERSION {
        return Err(AsterError::validation_error(format!(
            "preview apps config version must be {PREVIEW_APPS_VERSION}",
        )));
    }

    if config.apps.is_empty() {
        return Err(AsterError::validation_error(
            "preview apps config must contain at least one app",
        ));
    }

    let mut seen_keys = HashSet::new();
    for app in &mut config.apps {
        app.key = normalize_non_empty("app key", &app.key)?;
        app.icon = app.icon.trim().to_string();
        app.label_i18n_key = normalize_optional_text(app.label_i18n_key.take());
        app.labels = normalize_locale_labels(std::mem::take(&mut app.labels))?;
        if app.label_i18n_key.is_none() && app.labels.is_empty() {
            return Err(AsterError::validation_error(format!(
                "preview app '{}' must provide labels or label_i18n_key",
                app.key
            )));
        }

        if !seen_keys.insert(app.key.clone()) {
            return Err(AsterError::validation_error(format!(
                "duplicate preview app key '{}'",
                app.key
            )));
        }

        validate_preview_app_config(app)?;
    }

    for builtin_key in REQUIRED_BUILTIN_PREVIEW_APP_KEYS {
        if !seen_keys.contains(*builtin_key) {
            return Err(AsterError::validation_error(format!(
                "built-in preview app '{}' cannot be removed",
                builtin_key
            )));
        }
    }

    let app_keys: HashSet<&str> = config.apps.iter().map(|app| app.key.as_str()).collect();
    for rule in &mut config.rules {
        normalize_match_list(&mut rule.matches.extensions, normalize_extension)?;
        normalize_match_list(&mut rule.matches.mime_types, normalize_mime)?;
        normalize_match_list(&mut rule.matches.mime_prefixes, normalize_mime_prefix)?;
        normalize_match_list(&mut rule.matches.categories, normalize_category)?;
        normalize_match_list(&mut rule.apps, |value| {
            normalize_non_empty("rule app", value)
        })?;

        if rule.apps.is_empty() {
            return Err(AsterError::validation_error(
                "preview app rule must reference at least one app",
            ));
        }

        if let Some(default_app) = rule.default_app.take() {
            let normalized_default = normalize_non_empty("rule default_app", &default_app)?;
            if !rule.apps.iter().any(|key| key == &normalized_default) {
                return Err(AsterError::validation_error(format!(
                    "preview app rule default_app '{}' must be present in apps",
                    normalized_default
                )));
            }
            rule.default_app = Some(normalized_default);
        }

        let mut seen_rule_apps = HashSet::new();
        for app_key in &rule.apps {
            if !seen_rule_apps.insert(app_key.clone()) {
                return Err(AsterError::validation_error(format!(
                    "preview app rule contains duplicate app '{}'",
                    app_key
                )));
            }
            if !app_keys.contains(app_key.as_str()) {
                return Err(AsterError::validation_error(format!(
                    "preview app rule references unknown app '{}'",
                    app_key
                )));
            }
        }
    }

    Ok(())
}

fn validate_preview_app_config(app: &mut PublicPreviewAppDefinition) -> Result<()> {
    let provider = app.provider;
    ensure_supported_provider(app, provider)?;

    if is_table_preview_app_key(&app.key) {
        let delimiter = app
            .config
            .delimiter
            .take()
            .unwrap_or_else(|| DEFAULT_TABLE_PREVIEW_DELIMITER.to_string());
        app.config.delimiter = Some(normalize_table_delimiter(&delimiter)?);

        return Ok(());
    }

    normalize_allowed_origins(&mut app.config.allowed_origins)?;
    normalize_form_fields(&mut app.config.form_fields);

    match provider {
        PreviewAppProvider::Builtin => {}
        PreviewAppProvider::UrlTemplate => {
            if app.config.mode.is_none() {
                return Err(AsterError::validation_error(format!(
                    "preview app '{}' url_template provider requires config.mode",
                    app.key
                )));
            }

            let url_template = app.config.url_template.take().ok_or_else(|| {
                AsterError::validation_error(format!(
                    "preview app '{}' url_template provider requires config.url_template",
                    app.key
                ))
            })?;
            app.config.url_template = Some(normalize_non_empty("url_template", &url_template)?);
        }
        PreviewAppProvider::Wopi => {
            if app.config.mode.is_none() {
                return Err(AsterError::validation_error(format!(
                    "preview app '{}' wopi provider requires config.mode",
                    app.key
                )));
            }

            app.config.action = normalize_optional_non_empty("action", app.config.action.take())
                .map(|action| action.to_ascii_lowercase());
            app.config.action_url =
                normalize_optional_non_empty("action_url", app.config.action_url.take());
            app.config.action_url_template = normalize_optional_non_empty(
                "action_url_template",
                app.config.action_url_template.take(),
            );
            app.config.discovery_url =
                normalize_optional_non_empty("discovery_url", app.config.discovery_url.take());
        }
    }

    Ok(())
}

fn ensure_supported_provider(
    app: &PublicPreviewAppDefinition,
    provider: PreviewAppProvider,
) -> Result<()> {
    if provider == PreviewAppProvider::Builtin && !is_required_builtin_preview_app_key(&app.key) {
        return Err(AsterError::validation_error(format!(
            "preview app '{}' cannot use builtin provider",
            app.key
        )));
    }

    if is_required_builtin_preview_app_key(&app.key) && provider != PreviewAppProvider::Builtin {
        return Err(AsterError::validation_error(format!(
            "preview app '{}' must use provider 'builtin'",
            app.key
        )));
    }

    Ok(())
}

fn normalize_match_list<F>(items: &mut Vec<String>, normalize: F) -> Result<()>
where
    F: Fn(&str) -> Result<String>,
{
    let mut unique = BTreeSet::new();
    for item in std::mem::take(items) {
        unique.insert(normalize(&item)?);
    }
    *items = unique.into_iter().collect();
    Ok(())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_locale_labels(labels: BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();

    for (locale, label) in labels {
        normalized.insert(
            normalize_locale_key(&locale)?,
            normalize_non_empty("label", &label)?,
        );
    }

    Ok(normalized)
}

fn normalize_locale_key(value: &str) -> Result<String> {
    let locale = normalize_non_empty("label locale", value)?
        .to_ascii_lowercase()
        .replace('_', "-");

    if !locale
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(AsterError::validation_error(format!(
            "unsupported label locale '{locale}'",
        )));
    }

    Ok(locale)
}

fn normalize_non_empty(field: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(format!(
            "{field} must not be empty"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_extension(value: &str) -> Result<String> {
    let normalized = normalize_non_empty("extension", value)?;
    Ok(normalized.trim_start_matches('.').to_ascii_lowercase())
}

fn normalize_table_delimiter(value: &str) -> Result<String> {
    match value.trim() {
        "auto" => Ok("auto".to_string()),
        "," => Ok(",".to_string()),
        "\t" => Ok("\t".to_string()),
        ";" => Ok(";".to_string()),
        "|" => Ok("|".to_string()),
        _ => Err(AsterError::validation_error(
            "table delimiter must be one of: auto, ',', '\\t', ';', '|'",
        )),
    }
}

fn normalize_mime(value: &str) -> Result<String> {
    Ok(normalize_non_empty("mime type", value)?.to_ascii_lowercase())
}

fn normalize_mime_prefix(value: &str) -> Result<String> {
    Ok(normalize_non_empty("mime prefix", value)?.to_ascii_lowercase())
}

fn normalize_category(value: &str) -> Result<String> {
    let category = normalize_non_empty("category", value)?.to_ascii_lowercase();
    match category.as_str() {
        "image" | "video" | "audio" | "pdf" | "markdown" | "csv" | "tsv" | "json" | "xml"
        | "text" | "archive" | "document" | "spreadsheet" | "presentation" | "unknown" => {
            Ok(category)
        }
        _ => Err(AsterError::validation_error(format!(
            "unsupported preview category '{category}'",
        ))),
    }
}

fn normalize_optional_non_empty(field: &str, value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(normalize_non_empty(field, trimmed).ok()?)
        }
    })
}

fn normalize_allowed_origins(origins: &mut Vec<String>) -> Result<()> {
    let mut normalized = Vec::new();
    for origin in std::mem::take(origins) {
        let origin = normalize_non_empty("allowed_origin", &origin)?;
        if !normalized.contains(&origin) {
            normalized.push(origin);
        }
    }
    *origins = normalized;
    Ok(())
}

fn normalize_form_fields(form_fields: &mut BTreeMap<String, String>) {
    let mut normalized = BTreeMap::new();
    for (key, value) in std::mem::take(form_fields) {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        normalized.insert(key.to_string(), value.to_string());
    }
    *form_fields = normalized;
}

const fn default_preview_apps_version() -> i32 {
    PREVIEW_APPS_VERSION
}

const fn default_true() -> bool {
    true
}

fn builtin_app(
    key: &str,
    icon: &str,
    labels: BTreeMap<String, String>,
) -> PublicPreviewAppDefinition {
    builtin_app_with_config(key, icon, labels, PublicPreviewAppConfig::default())
}

fn builtin_app_with_config(
    key: &str,
    icon: &str,
    labels: BTreeMap<String, String>,
    config: PublicPreviewAppConfig,
) -> PublicPreviewAppDefinition {
    app_with_config(PreviewAppProvider::Builtin, key, icon, labels, config)
}

fn url_template_app(
    key: &str,
    icon: &str,
    labels: BTreeMap<String, String>,
    config: PublicPreviewAppConfig,
) -> PublicPreviewAppDefinition {
    app_with_config(PreviewAppProvider::UrlTemplate, key, icon, labels, config)
}

fn app_with_config(
    provider: PreviewAppProvider,
    key: &str,
    icon: &str,
    labels: BTreeMap<String, String>,
    config: PublicPreviewAppConfig,
) -> PublicPreviewAppDefinition {
    PublicPreviewAppDefinition {
        key: key.to_string(),
        provider,
        icon: icon.to_string(),
        enabled: true,
        label_i18n_key: None,
        labels,
        config,
    }
}

fn labels(primary: (&str, &str), secondary: (&str, &str)) -> BTreeMap<String, String> {
    BTreeMap::from([
        (primary.0.to_string(), primary.1.to_string()),
        (secondary.0.to_string(), secondary.1.to_string()),
    ])
}

fn is_table_preview_app_key(key: &str) -> bool {
    key.trim() == BUILTIN_TABLE_PREVIEW_APP_KEY
}

fn is_required_builtin_preview_app_key(key: &str) -> bool {
    REQUIRED_BUILTIN_PREVIEW_APP_KEYS.contains(&key)
}

fn category_rule(category: &str, apps: &[&str], default_app: Option<&str>) -> PublicPreviewAppRule {
    PublicPreviewAppRule {
        matches: PublicPreviewAppMatch {
            categories: vec![category.to_string()],
            ..Default::default()
        },
        apps: apps.iter().map(|value| value.to_string()).collect(),
        default_app: default_app.map(ToString::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PREVIEW_APPS_CONFIG_KEY, PreviewAppProvider, PreviewOpenMode, PublicPreviewAppConfig,
        default_public_preview_apps, normalize_public_preview_apps_config_value,
        parse_public_preview_apps_config,
    };
    use serde_json::{Value, json};

    #[test]
    fn default_preview_apps_serialize_and_parse() {
        let raw = serde_json::to_string(&default_public_preview_apps()).unwrap();
        let parsed = parse_public_preview_apps_config(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert!(parsed.apps.iter().any(|app| {
            app.key == "builtin.code"
                && app
                    .labels
                    .get("en")
                    .is_some_and(|label| label == "Source view")
                && app
                    .labels
                    .get("zh")
                    .is_some_and(|label| label == "源码视图")
        }));
        assert!(parsed.rules.iter().any(|rule| {
            rule.matches
                .categories
                .iter()
                .any(|category| category == "video")
                && rule.apps.iter().any(|app| app == "builtin.video")
        }));
    }

    #[test]
    fn preview_apps_json_is_normalized_and_pretty_printed() {
        let mut config = default_public_preview_apps();
        config.apps.push(super::PublicPreviewAppDefinition {
            key: " custom.viewer ".to_string(),
            provider: PreviewAppProvider::UrlTemplate,
            icon: "Globe".to_string(),
            enabled: true,
            label_i18n_key: None,
            labels: std::collections::BTreeMap::from([
                (" EN ".to_string(), " Viewer ".to_string()),
                ("zh".to_string(), " 查看器 ".to_string()),
            ]),
            config: PublicPreviewAppConfig {
                mode: Some(PreviewOpenMode::Iframe),
                url_template: Some(
                    " https://viewer.example.com/?url={{file_preview_url}} ".to_string(),
                ),
                allowed_origins: vec![
                    " https://viewer.example.com ".to_string(),
                    "https://viewer.example.com".to_string(),
                ],
                ..Default::default()
            },
        });
        config.rules.push(super::PublicPreviewAppRule {
            matches: super::PublicPreviewAppMatch {
                categories: vec![" Video ".to_string()],
                ..Default::default()
            },
            apps: vec![" custom.viewer ".to_string()],
            default_app: Some(" custom.viewer ".to_string()),
        });

        let raw = serde_json::to_string(&config).unwrap();

        let normalized = normalize_public_preview_apps_config_value(&raw).unwrap();
        let normalized_json: Value = serde_json::from_str(&normalized).unwrap();

        assert!(
            normalized_json["apps"]
                .as_array()
                .is_some_and(|apps| apps.iter().any(|app| {
                    app["key"] == "custom.viewer"
                        && app["labels"]["en"] == "Viewer"
                        && app["labels"]["zh"] == "查看器"
                        && app["config"]["mode"] == "iframe"
                }))
        );
        assert_eq!(
            normalized_json["rules"][normalized_json["rules"].as_array().unwrap().len() - 1]["matches"]
                ["categories"],
            json!(["video"])
        );
    }

    #[test]
    fn preview_apps_reject_unknown_rule_app() {
        let mut config = default_public_preview_apps();
        config.rules = vec![super::PublicPreviewAppRule {
            matches: super::PublicPreviewAppMatch {
                categories: vec!["text".to_string()],
                ..Default::default()
            },
            apps: vec!["missing.app".to_string()],
            default_app: None,
        }];

        let raw = serde_json::to_string(&config).unwrap();

        let error = normalize_public_preview_apps_config_value(&raw).unwrap_err();
        assert!(error.to_string().contains("missing.app"));
    }

    #[test]
    fn preview_apps_constant_key_matches_expected_name() {
        assert_eq!(PREVIEW_APPS_CONFIG_KEY, "frontend_preview_apps_json");
    }

    #[test]
    fn preview_apps_require_explicit_provider_fields() {
        let mut raw = serde_json::to_value(default_public_preview_apps()).unwrap();
        raw["apps"]
            .as_array_mut()
            .and_then(|apps| apps.first_mut())
            .and_then(Value::as_object_mut)
            .expect("default preview app should be an object")
            .remove("provider");

        let error = normalize_public_preview_apps_config_value(&raw.to_string()).unwrap_err();
        assert!(error.to_string().contains("missing field `provider`"));
    }

    #[test]
    fn preview_apps_allow_removing_external_viewers_but_not_core_builtins() {
        let raw = json!({
            "version": 1,
            "apps": [
                {
                    "key": "builtin.image",
                    "provider": "builtin",
                    "icon": "Eye",
                    "labels": { "en": "Image preview" }
                },
                {
                    "key": "builtin.video",
                    "provider": "builtin",
                    "icon": "Monitor",
                    "labels": { "en": "Video preview" }
                },
                {
                    "key": "builtin.audio",
                    "provider": "builtin",
                    "icon": "FileAudio",
                    "labels": { "en": "Audio preview" }
                },
                {
                    "key": "builtin.pdf",
                    "provider": "builtin",
                    "icon": "FileText",
                    "labels": { "en": "PDF preview" }
                },
                {
                    "key": "builtin.markdown",
                    "provider": "builtin",
                    "icon": "Eye",
                    "labels": { "en": "Markdown preview" }
                },
                {
                    "key": "builtin.table",
                    "provider": "builtin",
                    "icon": "Table",
                    "labels": { "en": "Table preview" },
                    "config": { "delimiter": "auto" }
                },
                {
                    "key": "builtin.formatted_json",
                    "provider": "builtin",
                    "icon": "BracketsCurly",
                    "labels": { "en": "Formatted view" }
                },
                {
                    "key": "builtin.formatted_xml",
                    "provider": "builtin",
                    "icon": "BracketsCurly",
                    "labels": { "en": "Formatted view" }
                },
                {
                    "key": "builtin.code",
                    "provider": "builtin",
                    "icon": "FileCode",
                    "labels": { "en": "Source view" }
                },
                {
                    "key": "builtin.try_text",
                    "provider": "builtin",
                    "icon": "FileCode",
                    "labels": { "en": "Open as text" }
                }
            ],
            "rules": [
                {
                    "apps": ["builtin.code"],
                    "matches": { "categories": ["text"] }
                }
            ]
        })
        .to_string();

        assert!(normalize_public_preview_apps_config_value(&raw).is_ok());
    }

    #[test]
    fn preview_apps_allow_empty_icon_and_trim_it() {
        let raw = json!({
            "version": 1,
            "apps": [
                {
                    "key": "builtin.image",
                    "provider": "builtin",
                    "icon": "Eye",
                    "labels": { "en": "Image preview" }
                },
                {
                    "key": "builtin.video",
                    "provider": "builtin",
                    "icon": "Monitor",
                    "labels": { "en": "Video preview" }
                },
                {
                    "key": "builtin.audio",
                    "provider": "builtin",
                    "icon": "FileAudio",
                    "labels": { "en": "Audio preview" }
                },
                {
                    "key": "builtin.pdf",
                    "provider": "builtin",
                    "icon": "FileText",
                    "labels": { "en": "PDF preview" }
                },
                {
                    "key": "builtin.markdown",
                    "provider": "builtin",
                    "icon": "Eye",
                    "labels": { "en": "Markdown preview" }
                },
                {
                    "key": "builtin.table",
                    "provider": "builtin",
                    "icon": "Table",
                    "labels": { "en": "Table preview" },
                    "config": { "delimiter": "auto" }
                },
                {
                    "key": "builtin.formatted_json",
                    "provider": "builtin",
                    "icon": "BracketsCurly",
                    "labels": { "en": "Formatted view" }
                },
                {
                    "key": "builtin.formatted_xml",
                    "provider": "builtin",
                    "icon": "BracketsCurly",
                    "labels": { "en": "Formatted view" }
                },
                {
                    "key": "builtin.code",
                    "provider": "builtin",
                    "icon": "FileCode",
                    "labels": { "en": "Source view" }
                },
                {
                    "key": "builtin.try_text",
                    "provider": "builtin",
                    "icon": "FileCode",
                    "labels": { "en": "Open as text" }
                },
                {
                    "key": "custom.viewer",
                    "provider": "url_template",
                    "icon": "   ",
                    "labels": { "en": "Viewer" },
                    "config": {
                        "mode": "iframe",
                        "url_template": "https://viewer.example.com/?src={{file_preview_url}}"
                    }
                }
            ],
            "rules": [
                {
                    "apps": ["builtin.code", "custom.viewer"],
                    "default_app": "custom.viewer",
                    "matches": { "categories": ["text"] }
                }
            ]
        })
        .to_string();

        let normalized = normalize_public_preview_apps_config_value(&raw).unwrap();
        let normalized_json: Value = serde_json::from_str(&normalized).unwrap();

        assert!(normalized_json["apps"].as_array().is_some_and(|apps| {
            apps.iter()
                .any(|app| app["key"] == "custom.viewer" && app["icon"] == "")
        }));
    }
}
