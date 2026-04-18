//! WOPI 服务子模块：`discovery`。

use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use moka::future::Cache;
use reqwest::Url;
use xmltree::{Element, XMLNode};

use crate::config::{cors, wopi};
use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::preview_app_service;

use super::proof::{WopiProofKeySet, parse_wopi_proof_key_set, validate_wopi_proof};
use super::targets::file_extension;
use super::types::{DiscoveredWopiPreviewApp, WopiRequestSource};

static DISCOVERY_CACHE: LazyLock<Cache<String, CachedWopiDiscovery>> =
    LazyLock::new(|| Cache::builder().max_capacity(128).build());

static DISCOVERY_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(StdDuration::from_secs(5))
        .build()
        .expect("wopi discovery client should initialize")
});

const DISCOVERY_ACTION_PRIORITY: &[&str] = &[
    "embededit",
    "edit",
    "mobileedit",
    "embedview",
    "view",
    "mobileview",
];

#[derive(Debug, Clone)]
pub(crate) struct WopiAppConfig {
    pub(crate) action: String,
    pub(crate) action_url: Option<String>,
    pub(crate) discovery_url: Option<String>,
    pub(crate) form_fields: std::collections::BTreeMap<String, String>,
    pub(crate) mode: preview_app_service::PreviewOpenMode,
}

#[derive(Debug, Clone)]
struct WopiDiscoveryAction {
    action: String,
    app_icon_url: Option<String>,
    app_name: Option<String>,
    ext: Option<String>,
    mime: Option<String>,
    urlsrc: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WopiDiscovery {
    actions: Vec<WopiDiscoveryAction>,
    proof_keys: Option<WopiProofKeySet>,
}

#[derive(Debug, Clone)]
struct CachedWopiDiscovery {
    discovery: WopiDiscovery,
    cached_at: DateTime<Utc>,
}

pub fn allowed_origins(state: &AppState) -> Vec<String> {
    let mut origins = Vec::new();

    for app in preview_app_service::get_public_preview_apps(state).apps {
        if app.provider != preview_app_service::PreviewAppProvider::Wopi {
            continue;
        }
        for origin in trusted_origins_for_app(&app) {
            push_unique(&mut origins, origin);
        }
    }

    origins
}

pub async fn discover_preview_apps(
    state: &AppState,
    discovery_url: &str,
) -> Result<Vec<DiscoveredWopiPreviewApp>> {
    let discovery = load_discovery(state, discovery_url).await?;
    let apps = build_discovered_preview_apps(&discovery);
    if apps.is_empty() {
        return Err(AsterError::validation_error(
            "WOPI discovery did not expose any importable preview apps",
        ));
    }
    Ok(apps)
}

pub(crate) fn parse_wopi_app_config(
    app: &preview_app_service::PublicPreviewAppDefinition,
) -> Result<WopiAppConfig> {
    if app.provider != preview_app_service::PreviewAppProvider::Wopi {
        return Err(AsterError::validation_error(format!(
            "preview app '{}' is not a WOPI provider",
            app.key
        )));
    }

    let mode = app.config.mode.ok_or_else(|| {
        AsterError::validation_error(format!(
            "preview app '{}' WOPI provider requires config.mode",
            app.key
        ))
    })?;

    let action = app
        .config
        .action
        .as_deref()
        .unwrap_or("edit")
        .to_ascii_lowercase();

    let action_url = app
        .config
        .action_url
        .clone()
        .or_else(|| app.config.action_url_template.clone());
    let discovery_url = app.config.discovery_url.clone();
    if action_url.is_none() && discovery_url.is_none() {
        return Err(AsterError::validation_error(format!(
            "preview app '{}' WOPI provider requires config.action_url or config.discovery_url",
            app.key
        )));
    }

    Ok(WopiAppConfig {
        action,
        action_url,
        discovery_url,
        form_fields: app.config.form_fields.clone(),
        mode,
    })
}

pub(crate) async fn resolve_action_url(
    state: &AppState,
    app_config: &WopiAppConfig,
    file: &file::Model,
    wopi_src: &str,
) -> Result<String> {
    if let Some(action_url) = app_config.action_url.as_deref() {
        return expand_action_url(action_url, wopi_src);
    }

    let discovery_url = app_config
        .discovery_url
        .as_deref()
        .ok_or_else(|| AsterError::validation_error("missing WOPI discovery URL"))?;
    let discovery = load_discovery(state, discovery_url).await?;
    let extension = file_extension(&file.name);
    let urlsrc = resolve_discovery_action_url(
        &discovery,
        &app_config.action,
        extension.as_deref(),
        &file.mime_type,
    )
    .ok_or_else(|| {
        AsterError::validation_error(format!(
            "WOPI discovery has no compatible action for '{}' (preferred action '{}')",
            file.name, app_config.action
        ))
    })?;
    append_wopi_src(&urlsrc, wopi_src)
}

pub(crate) async fn load_discovery(state: &AppState, discovery_url: &str) -> Result<WopiDiscovery> {
    let cached = DISCOVERY_CACHE.get(discovery_url).await;
    if let Some(cached) = cached.as_ref()
        && cached.cached_at + discovery_cache_ttl(&state.runtime_config) > Utc::now()
    {
        return Ok(cached.discovery.clone());
    }

    let response = match DISCOVERY_CLIENT
        .get(discovery_url)
        .send()
        .await
        .map_aster_err_ctx(
            "failed to fetch WOPI discovery",
            AsterError::validation_error,
        ) {
        Ok(response) => response,
        Err(error) => {
            if let Some(cached) = cached.as_ref() {
                tracing::warn!(
                    discovery_url,
                    error = %error,
                    "using stale WOPI discovery cache after refresh failure"
                );
                return Ok(cached.discovery.clone());
            }
            return Err(error);
        }
    };
    if !response.status().is_success() {
        if let Some(cached) = cached.as_ref() {
            tracing::warn!(
                discovery_url,
                status = %response.status(),
                "using stale WOPI discovery cache after non-success refresh"
            );
            return Ok(cached.discovery.clone());
        }
        return Err(AsterError::validation_error(format!(
            "WOPI discovery returned HTTP {}",
            response.status()
        )));
    }

    let body = response.text().await.map_aster_err_ctx(
        "failed to read WOPI discovery",
        AsterError::validation_error,
    )?;
    let parsed = match parse_discovery_xml(&body) {
        Ok(parsed) => parsed,
        Err(error) => {
            if let Some(cached) = cached.as_ref() {
                tracing::warn!(
                    discovery_url,
                    error = %error,
                    "using stale WOPI discovery cache after parse failure"
                );
                return Ok(cached.discovery.clone());
            }
            return Err(error);
        }
    };
    DISCOVERY_CACHE
        .insert(
            discovery_url.to_string(),
            CachedWopiDiscovery {
                discovery: parsed.clone(),
                cached_at: Utc::now(),
            },
        )
        .await;
    Ok(parsed)
}

pub(crate) fn parse_discovery_xml(xml: &str) -> Result<WopiDiscovery> {
    let root = Element::parse(xml.as_bytes())
        .map_aster_err_ctx("invalid WOPI discovery XML", AsterError::validation_error)?;
    let mut actions = Vec::new();
    let mut proof_keys = None;
    collect_discovery_proof_keys(&root, &mut proof_keys)?;
    collect_discovery_actions(&root, None, None, &mut actions);
    if actions.is_empty() {
        return Err(AsterError::validation_error(
            "WOPI discovery did not expose any actions",
        ));
    }

    Ok(WopiDiscovery {
        actions,
        proof_keys,
    })
}

fn collect_discovery_proof_keys(
    element: &Element,
    out: &mut Option<WopiProofKeySet>,
) -> Result<()> {
    if element.name.eq_ignore_ascii_case("proof-key") {
        let current_modulus = element_attribute(element, "modulus")
            .ok_or_else(|| AsterError::validation_error("WOPI proof-key is missing modulus"))?;
        let current_exponent = element_attribute(element, "exponent")
            .ok_or_else(|| AsterError::validation_error("WOPI proof-key is missing exponent"))?;
        let parsed = parse_wopi_proof_key_set(
            current_modulus,
            current_exponent,
            element_attribute(element, "oldmodulus"),
            element_attribute(element, "oldexponent"),
        )?;
        if out.replace(parsed).is_some() {
            return Err(AsterError::validation_error(
                "WOPI discovery contains multiple proof-key elements",
            ));
        }
    }

    for child in &element.children {
        if let XMLNode::Element(child) = child {
            collect_discovery_proof_keys(child, out)?;
        }
    }

    Ok(())
}

fn collect_discovery_actions(
    element: &Element,
    app_name: Option<&str>,
    app_icon_url: Option<&str>,
    out: &mut Vec<WopiDiscoveryAction>,
) {
    let (next_app_name, next_app_icon_url) = if element.name.eq_ignore_ascii_case("app") {
        (
            element_attribute(element, "name").or(app_name),
            element_attribute(element, "favIconUrl").or(app_icon_url),
        )
    } else {
        (app_name, app_icon_url)
    };

    if element.name.eq_ignore_ascii_case("action") {
        let action =
            element_attribute(element, "name").map(|value| value.trim().to_ascii_lowercase());
        let urlsrc = element_attribute(element, "urlsrc").map(|value| value.trim().to_string());
        if let (Some(action), Some(urlsrc)) = (action, urlsrc)
            && !action.is_empty()
            && !urlsrc.is_empty()
        {
            let ext = element_attribute(element, "ext")
                .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|value| !value.is_empty());
            let mime = next_app_name
                .map(str::trim)
                .filter(|value| value.contains('/'))
                .map(|value| value.to_ascii_lowercase());
            out.push(WopiDiscoveryAction {
                action,
                app_icon_url: next_app_icon_url.map(str::trim).map(ToString::to_string),
                app_name: next_app_name.map(str::trim).map(ToString::to_string),
                ext,
                mime,
                urlsrc,
            });
        }
    }

    for child in &element.children {
        if let XMLNode::Element(child) = child {
            collect_discovery_actions(child, next_app_name, next_app_icon_url, out);
        }
    }
}

fn element_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element.attributes.iter().find_map(|(key, value)| {
        if key.eq_ignore_ascii_case(name) {
            Some(value.as_str())
        } else {
            None
        }
    })
}

impl WopiDiscovery {
    pub(crate) fn find_action_url(
        &self,
        action: &str,
        extension: Option<&str>,
        mime_type: &str,
    ) -> Option<String> {
        let action = action.to_ascii_lowercase();
        let extension = extension.map(|value| value.to_ascii_lowercase());
        let mime_type = mime_type.trim().to_ascii_lowercase();

        self.actions
            .iter()
            .find(|item| item.action == action && item.ext.as_deref() == extension.as_deref())
            .or_else(|| {
                self.actions.iter().find(|item| {
                    item.action == action && item.mime.as_deref() == Some(mime_type.as_str())
                })
            })
            .or_else(|| {
                self.actions
                    .iter()
                    .find(|item| item.action == action && item.ext.as_deref() == Some("*"))
            })
            .map(|item| item.urlsrc.clone())
    }

    pub(crate) fn proof_keys(&self) -> Option<&WopiProofKeySet> {
        self.proof_keys.as_ref()
    }
}

pub(crate) fn resolve_discovery_action_url(
    discovery: &WopiDiscovery,
    requested_action: &str,
    extension: Option<&str>,
    mime_type: &str,
) -> Option<String> {
    let preferred_actions = preferred_discovery_actions(requested_action);

    preferred_actions
        .iter()
        .find_map(|action| discovery.find_action_url(action, extension, mime_type))
}

fn preferred_discovery_actions(requested_action: &str) -> Vec<String> {
    let normalized = requested_action.trim().to_ascii_lowercase();
    let mut actions = Vec::new();

    if !normalized.is_empty() && !is_known_discovery_action(&normalized) {
        actions.push(normalized);
    }

    for candidate in DISCOVERY_ACTION_PRIORITY {
        if actions.iter().any(|existing| existing == candidate) {
            continue;
        }
        actions.push((*candidate).to_string());
    }

    actions
}

fn is_known_discovery_action(action: &str) -> bool {
    DISCOVERY_ACTION_PRIORITY.contains(&action)
}

pub(crate) fn build_discovered_preview_apps(
    discovery: &WopiDiscovery,
) -> Vec<DiscoveredWopiPreviewApp> {
    #[derive(Debug, Clone)]
    struct DiscoveryGroup {
        icon_url: Option<String>,
        label: String,
        actions: Vec<WopiDiscoveryAction>,
    }

    let mut groups = Vec::<DiscoveryGroup>::new();
    for action in &discovery.actions {
        let label = action
            .app_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("WOPI");

        if let Some(group) = groups.iter_mut().find(|group| group.label == label) {
            group.actions.push(action.clone());
            if group.icon_url.is_none() {
                group.icon_url = action.app_icon_url.clone();
            }
            continue;
        }

        groups.push(DiscoveryGroup {
            icon_url: action.app_icon_url.clone(),
            label: label.to_string(),
            actions: vec![action.clone()],
        });
    }

    let mut results = Vec::new();
    let mut used_suffixes = std::collections::HashSet::new();

    for group in groups {
        let action_name = DISCOVERY_ACTION_PRIORITY
            .iter()
            .find_map(|candidate| {
                let has_extensions = group.actions.iter().any(|action| {
                    action.action == *candidate
                        && action
                            .ext
                            .as_deref()
                            .is_some_and(|ext| !ext.is_empty() && ext != "*")
                });
                has_extensions.then_some((*candidate).to_string())
            })
            .or_else(|| {
                group.actions.iter().find_map(|action| {
                    action
                        .ext
                        .as_deref()
                        .is_some_and(|ext| !ext.is_empty() && ext != "*")
                        .then(|| action.action.clone())
                })
            });

        let Some(action_name) = action_name else {
            continue;
        };

        let mut extensions = Vec::new();
        for action in &group.actions {
            let should_collect_extension = if is_known_discovery_action(&action_name) {
                is_known_discovery_action(&action.action)
            } else {
                action.action == action_name
            };

            if !should_collect_extension {
                continue;
            }
            if let Some(ext) = action.ext.as_deref()
                && !ext.is_empty()
                && ext != "*"
            {
                push_unique(&mut extensions, ext.to_string());
            }
        }

        if extensions.is_empty() {
            continue;
        }

        let mut key_suffix = slugify_discovery_app_name(&group.label);
        if key_suffix.is_empty() {
            key_suffix = "app".to_string();
        }

        if !used_suffixes.insert(key_suffix.clone()) {
            let base = key_suffix.clone();
            let mut index = 2;
            loop {
                let candidate = format!("{base}_{index}");
                if used_suffixes.insert(candidate.clone()) {
                    key_suffix = candidate;
                    break;
                }
                index += 1;
            }
        }

        results.push(DiscoveredWopiPreviewApp {
            action: action_name,
            extensions,
            icon_url: group.icon_url,
            key_suffix,
            label: group.label,
        });
    }

    results
}

fn slugify_discovery_app_name(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator && !slug.is_empty() {
            slug.push('_');
            previous_was_separator = true;
        }
    }

    slug.trim_matches('_').to_string()
}

pub(crate) fn expand_action_url(raw: &str, wopi_src: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(
            "WOPI action_url must not be empty",
        ));
    }

    let wopi_src_encoded = urlencoding::encode(wopi_src);
    let resolved = trimmed
        .replace("{{wopi_src}}", &wopi_src_encoded)
        .replace("{{WOPISrc}}", &wopi_src_encoded);
    if resolved.contains("{{wopi_src}}") || resolved.contains("{{WOPISrc}}") {
        return Err(AsterError::validation_error(
            "WOPI action_url contains an unresolved WOPISrc placeholder",
        ));
    }

    let resolved = expand_discovery_url_placeholders(&resolved, &wopi_src_encoded);
    if resolved.contains('<') || resolved.contains('>') {
        return Err(AsterError::validation_error(
            "WOPI action_url contains unresolved discovery placeholders",
        ));
    }

    if resolved == trimmed {
        return append_wopi_src(trimmed, wopi_src);
    }

    Url::parse(&resolved).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI action_url: {error}"))
    })?;
    append_wopi_src_if_missing(&resolved, wopi_src)
}

fn expand_discovery_url_placeholders(raw: &str, wopi_src_encoded: &str) -> String {
    let mut output = String::with_capacity(raw.len() + wopi_src_encoded.len());
    let mut index = 0;

    while let Some(start_offset) = raw[index..].find('<') {
        let start = index + start_offset;
        output.push_str(&raw[index..start]);

        let Some(end_offset) = raw[start + 1..].find('>') else {
            output.push_str(&raw[start..]);
            return output;
        };
        let end = start + 1 + end_offset;
        let placeholder = &raw[start + 1..end];
        if let Some(replacement) = resolve_discovery_placeholder(placeholder, wopi_src_encoded) {
            output.push_str(&replacement);
        }
        index = end + 1;
    }

    output.push_str(&raw[index..]);
    output
}

fn resolve_discovery_placeholder(placeholder: &str, wopi_src_encoded: &str) -> Option<String> {
    let trimmed = placeholder.trim();
    let (key, value) = trimmed.split_once('=')?;
    let key = key.trim();
    let value = value.trim().trim_end_matches('&').trim();
    if key.is_empty() {
        return None;
    }

    if key.eq_ignore_ascii_case("wopisrc") || value.eq_ignore_ascii_case("wopi_source") {
        return Some(format!("{key}={wopi_src_encoded}&"));
    }

    None
}

fn append_wopi_src_if_missing(url: &str, wopi_src: &str) -> Result<String> {
    let parsed = Url::parse(url).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI action URL: {error}"))
    })?;
    let has_wopi_src = parsed
        .query_pairs()
        .any(|(key, _)| key.as_ref().eq_ignore_ascii_case("wopisrc"));
    if has_wopi_src {
        return Ok(parsed.to_string());
    }

    append_wopi_src(url, wopi_src)
}

pub(crate) fn append_wopi_src(url: &str, wopi_src: &str) -> Result<String> {
    let mut parsed = Url::parse(url).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI action URL: {error}"))
    })?;
    parsed.query_pairs_mut().append_pair("WOPISrc", wopi_src);
    Ok(parsed.to_string())
}

fn discovery_cache_ttl(runtime_config: &crate::config::RuntimeConfig) -> Duration {
    let ttl_secs = wopi::discovery_cache_ttl_secs(runtime_config);
    Duration::seconds(i64::try_from(ttl_secs).unwrap_or(i64::MAX))
}

fn origin_from_url(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    let host = parsed.host_str()?.to_ascii_lowercase();
    let port = parsed
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    cors::normalize_origin(&format!("{scheme}://{host}{port}"), false).ok()
}

pub(crate) fn trusted_origins_for_app(
    app: &preview_app_service::PublicPreviewAppDefinition,
) -> Vec<String> {
    let mut origins = Vec::new();

    for origin in &app.config.allowed_origins {
        if let Ok(origin) = cors::normalize_origin(origin, false) {
            push_unique(&mut origins, origin);
        }
    }

    for raw in [
        app.config.action_url.as_deref(),
        app.config.action_url_template.as_deref(),
        app.config.discovery_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(origin) = origin_from_url(raw) {
            push_unique(&mut origins, origin);
        }
    }

    origins
}

pub(crate) fn ensure_request_source_allowed(
    app: &preview_app_service::PublicPreviewAppDefinition,
    request_source: &WopiRequestSource<'_>,
) -> Result<()> {
    // 这里只做“配置级来源收敛”，用于挡掉明显不可信的 Origin / Referer。
    // 真正的 Microsoft 365 proof-key 验签在 `ensure_request_proof_valid()`，
    // 两层一起才构成完整的 WOPI 请求来源校验。
    let trusted_origins = trusted_origins_for_app(app);
    if trusted_origins.is_empty() {
        return Ok(());
    }

    if let Some(origin) = request_source
        .origin
        .filter(|value| !value.trim().is_empty())
        .map(|value| cors::normalize_origin(value, false))
        .transpose()
        .map_aster_err_with(|| AsterError::validation_error("invalid Origin header"))?
    {
        if trusted_origins.iter().any(|allowed| allowed == &origin) {
            return Ok(());
        }
        return Err(AsterError::auth_forbidden("untrusted WOPI request origin"));
    }

    if let Some(referer) = request_source
        .referer
        .filter(|value| !value.trim().is_empty())
    {
        let referer_origin = origin_from_url(referer)
            .ok_or_else(|| AsterError::validation_error("invalid Referer header"))?;
        if trusted_origins
            .iter()
            .any(|allowed| allowed == &referer_origin)
        {
            return Ok(());
        }
        return Err(AsterError::auth_forbidden("untrusted WOPI request referer"));
    }

    Ok(())
}

pub(crate) async fn ensure_request_proof_valid(
    state: &AppState,
    app_config: &WopiAppConfig,
    access_token: &str,
    request_source: &WopiRequestSource<'_>,
) -> Result<()> {
    let Some(discovery_url) = app_config.discovery_url.as_deref() else {
        return Ok(());
    };
    let discovery = load_discovery(state, discovery_url).await?;
    let Some(proof_keys) = discovery.proof_keys() else {
        return Ok(());
    };
    let public_url = request_source.public_url.as_deref().ok_or_else(|| {
        AsterError::internal_error(
            "WOPI proof validation requires a configured public_site_url request URL",
        )
    })?;

    // discovery 提供了 proof-key 时，说明该 provider 期望按照微软定义的
    // proof 头部签名请求。这里必须在 access_token 解析成功后、业务落库前验签。
    validate_wopi_proof(
        proof_keys,
        access_token,
        public_url,
        request_source.proof,
        request_source.proof_old,
        request_source.timestamp,
        Utc::now(),
    )
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
