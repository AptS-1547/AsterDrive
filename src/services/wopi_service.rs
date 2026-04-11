use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use moka::future::Cache;
use reqwest::Url;
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;
use xmltree::{Element, XMLNode};

use crate::config::{cors, site_url, wopi};
use crate::db::repository::{file_repo, lock_repo, wopi_session_repo};
use crate::entities::{file, resource_lock, wopi_session};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    auth_service, file_service, lock_service, preview_app_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::EntityType;

static DISCOVERY_CACHE: LazyLock<Cache<String, CachedWopiDiscovery>> =
    LazyLock::new(|| Cache::builder().max_capacity(128).build());

static DISCOVERY_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(StdDuration::from_secs(5))
        .build()
        .expect("wopi discovery client should initialize")
});

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct WopiLaunchSession {
    pub access_token: String,
    pub access_token_ttl: i64,
    pub action_url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub form_fields: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<preview_app_service::PreviewOpenMode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WopiCheckFileInfo {
    pub base_file_name: String,
    pub owner_id: String,
    pub size: i64,
    pub user_id: String,
    pub user_can_not_write_relative: bool,
    pub user_can_rename: bool,
    pub user_can_write: bool,
    pub read_only: bool,
    pub supports_get_lock: bool,
    pub supports_locks: bool,
    pub supports_rename: bool,
    pub supports_update: bool,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct WopiConflict {
    pub current_lock: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum WopiPutFileResult {
    Success { item_version: String },
    Conflict(WopiConflict),
}

#[derive(Debug, Clone)]
pub enum WopiLockOperationResult {
    Success,
    Conflict(WopiConflict),
}

#[derive(Debug, Clone)]
struct WopiAppConfig {
    action: String,
    action_url: Option<String>,
    discovery_url: Option<String>,
    form_fields: BTreeMap<String, String>,
    mode: preview_app_service::PreviewOpenMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WopiAccessTokenPayload {
    actor_user_id: i64,
    session_version: i64,
    team_id: Option<i64>,
    file_id: i64,
    app_key: String,
    exp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WopiLockPayload {
    kind: String,
    app_key: String,
    lock: String,
}

#[derive(Debug, Clone)]
struct ResolvedWopiAccess {
    file: file::Model,
    payload: WopiAccessTokenPayload,
}

#[derive(Debug, Clone)]
struct ActiveWopiLock {
    lock: resource_lock::Model,
    payload: Option<WopiLockPayload>,
}

#[derive(Debug, Clone)]
struct WopiDiscoveryAction {
    action: String,
    ext: Option<String>,
    mime: Option<String>,
    urlsrc: String,
}

#[derive(Debug, Clone)]
struct WopiDiscovery {
    actions: Vec<WopiDiscoveryAction>,
}

#[derive(Debug, Clone)]
struct CachedWopiDiscovery {
    discovery: WopiDiscovery,
    cached_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WopiRequestSource<'a> {
    pub origin: Option<&'a str>,
    pub referer: Option<&'a str>,
}

pub(crate) async fn create_launch_session_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    app_key: &str,
) -> Result<WopiLaunchSession> {
    let file = workspace_storage_service::verify_file_access(state, scope, file_id).await?;
    let auth_snapshot = auth_service::get_auth_snapshot(state, scope.actor_user_id()).await?;
    let app = preview_app_service::get_public_preview_apps(state)
        .apps
        .into_iter()
        .find(|candidate| candidate.key == app_key)
        .ok_or_else(|| AsterError::record_not_found(format!("preview app '{app_key}'")))?;
    let app_config = parse_wopi_app_config(&app)?;

    let wopi_src = build_public_wopi_src(state, file.id)?;
    let action_url = resolve_action_url(state, &app_config, &file, &wopi_src).await?;
    let expires_at =
        Utc::now() + Duration::seconds(wopi::access_token_ttl_secs(&state.runtime_config));
    let access_token = create_access_token_session(
        state,
        &WopiAccessTokenPayload {
            actor_user_id: scope.actor_user_id(),
            session_version: auth_snapshot.session_version,
            team_id: scope.team_id(),
            file_id: file.id,
            app_key: app.key.clone(),
            exp: expires_at.timestamp(),
        },
    )
    .await?;

    Ok(WopiLaunchSession {
        access_token,
        access_token_ttl: expires_at.timestamp_millis(),
        action_url,
        form_fields: app_config.form_fields,
        mode: Some(app_config.mode),
    })
}

pub async fn check_file_info(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiCheckFileInfo> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let blob = file_repo::find_blob_by_id(&state.db, resolved.file.blob_id).await?;

    Ok(WopiCheckFileInfo {
        base_file_name: resolved.file.name.clone(),
        owner_id: resolved.file.user_id.to_string(),
        size: resolved.file.size,
        user_id: resolved.payload.actor_user_id.to_string(),
        user_can_not_write_relative: true,
        user_can_rename: false,
        user_can_write: true,
        read_only: false,
        supports_get_lock: false,
        supports_locks: true,
        supports_rename: false,
        supports_update: true,
        version: blob.hash,
    })
}

pub async fn get_file_contents(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    if_none_match: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<actix_web::HttpResponse> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let blob = file_repo::find_blob_by_id(&state.db, resolved.file.blob_id).await?;
    file_service::build_stream_response_with_disposition(
        state,
        &resolved.file,
        &blob,
        file_service::DownloadDisposition::Inline,
        if_none_match,
    )
    .await
}

pub async fn put_file_contents(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    body: actix_web::web::Bytes,
    requested_lock: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiPutFileResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    if let Some(conflict) =
        ensure_wopi_lock_matches(state, &resolved.payload, resolved.file.id, requested_lock).await?
    {
        return Ok(WopiPutFileResult::Conflict(conflict));
    }

    let (updated, item_version) = file_service::update_content_in_scope(
        state,
        scope_from_payload(&resolved.payload),
        resolved.file.id,
        body,
        None,
    )
    .await?;

    Ok(WopiPutFileResult::Success {
        item_version: item_version_if_present(updated.id, item_version),
    })
}

pub async fn lock_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let lock_value = normalize_wopi_lock_value(requested_lock)?;
    let active_lock = load_active_lock(state, resolved.file.id).await?;

    if let Some(active_lock) = active_lock {
        if let Some(payload) = active_lock.payload {
            if payload.app_key == resolved.payload.app_key && payload.lock == lock_value {
                refresh_lock_model(state, active_lock.lock).await?;
                return Ok(WopiLockOperationResult::Success);
            }

            return Ok(WopiLockOperationResult::Conflict(WopiConflict {
                current_lock: Some(payload.lock),
                reason: "file is locked by another WOPI session".to_string(),
            }));
        }

        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: None,
            reason: "file is locked outside WOPI".to_string(),
        }));
    }

    create_wopi_lock(state, &resolved.payload, &resolved.file, &lock_value).await?;
    Ok(WopiLockOperationResult::Success)
}

pub async fn refresh_lock(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let lock_value = normalize_wopi_lock_value(requested_lock)?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: None,
            reason: "file is not locked".to_string(),
        }));
    };

    match active_lock.payload {
        Some(payload)
            if payload.app_key == resolved.payload.app_key && payload.lock == lock_value =>
        {
            refresh_lock_model(state, active_lock.lock).await?;
            Ok(WopiLockOperationResult::Success)
        }
        Some(payload) => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(payload.lock),
            reason: "WOPI lock mismatch".to_string(),
        })),
        None => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: None,
            reason: "file is locked outside WOPI".to_string(),
        })),
    }
}

pub async fn unlock_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let lock_value = normalize_wopi_lock_value(requested_lock)?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: None,
            reason: "file is not locked".to_string(),
        }));
    };

    match active_lock.payload {
        Some(payload)
            if payload.app_key == resolved.payload.app_key && payload.lock == lock_value =>
        {
            lock_service::set_entity_locked(&state.db, EntityType::File, resolved.file.id, false)
                .await?;
            lock_repo::delete_by_id(&state.db, active_lock.lock.id).await?;
            Ok(WopiLockOperationResult::Success)
        }
        Some(payload) => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(payload.lock),
            reason: "WOPI lock mismatch".to_string(),
        })),
        None => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: None,
            reason: "file is locked outside WOPI".to_string(),
        })),
    }
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

fn item_version_if_present(_file_id: i64, item_version: String) -> String {
    item_version
}

fn parse_wopi_app_config(
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

async fn resolve_action_url(
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
    let urlsrc = discovery
        .find_action_url(&app_config.action, extension.as_deref(), &file.mime_type)
        .ok_or_else(|| {
            AsterError::validation_error(format!(
                "WOPI discovery has no '{}' action for '{}'",
                app_config.action, file.name
            ))
        })?;
    append_wopi_src(&urlsrc, wopi_src)
}

async fn load_discovery(state: &AppState, discovery_url: &str) -> Result<WopiDiscovery> {
    if let Some(cached) = DISCOVERY_CACHE.get(discovery_url).await
        && cached.cached_at + discovery_cache_ttl(&state.runtime_config) > Utc::now()
    {
        return Ok(cached.discovery);
    }

    let response = DISCOVERY_CLIENT
        .get(discovery_url)
        .send()
        .await
        .map_err(|error| {
            AsterError::validation_error(format!("failed to fetch WOPI discovery: {error}"))
        })?;
    if !response.status().is_success() {
        return Err(AsterError::validation_error(format!(
            "WOPI discovery returned HTTP {}",
            response.status()
        )));
    }

    let body = response.text().await.map_err(|error| {
        AsterError::validation_error(format!("failed to read WOPI discovery: {error}"))
    })?;
    let parsed = parse_discovery_xml(&body)?;
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

fn parse_discovery_xml(xml: &str) -> Result<WopiDiscovery> {
    let root = Element::parse(xml.as_bytes()).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI discovery XML: {error}"))
    })?;
    let mut actions = Vec::new();
    collect_discovery_actions(&root, None, &mut actions);
    if actions.is_empty() {
        return Err(AsterError::validation_error(
            "WOPI discovery did not expose any actions",
        ));
    }

    Ok(WopiDiscovery { actions })
}

fn collect_discovery_actions(
    element: &Element,
    app_name: Option<&str>,
    out: &mut Vec<WopiDiscoveryAction>,
) {
    let next_app_name = if element.name.eq_ignore_ascii_case("app") {
        element
            .attributes
            .get("name")
            .map(String::as_str)
            .or(app_name)
    } else {
        app_name
    };

    if element.name.eq_ignore_ascii_case("action") {
        let action = element
            .attributes
            .get("name")
            .map(|value| value.trim().to_ascii_lowercase());
        let urlsrc = element
            .attributes
            .get("urlsrc")
            .map(|value| value.trim().to_string());
        if let (Some(action), Some(urlsrc)) = (action, urlsrc)
            && !action.is_empty()
            && !urlsrc.is_empty()
        {
            let ext = element
                .attributes
                .get("ext")
                .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|value| !value.is_empty());
            let mime = next_app_name
                .map(str::trim)
                .filter(|value| value.contains('/'))
                .map(|value| value.to_ascii_lowercase());
            out.push(WopiDiscoveryAction {
                action,
                ext,
                mime,
                urlsrc,
            });
        }
    }

    for child in &element.children {
        if let XMLNode::Element(child) = child {
            collect_discovery_actions(child, next_app_name, out);
        }
    }
}

impl WopiDiscovery {
    fn find_action_url(
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
}

fn expand_action_url(raw: &str, wopi_src: &str) -> Result<String> {
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

    if resolved == trimmed {
        return append_wopi_src(trimmed, wopi_src);
    }

    Url::parse(&resolved).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI action_url: {error}"))
    })?;
    Ok(resolved)
}

fn append_wopi_src(url: &str, wopi_src: &str) -> Result<String> {
    let mut parsed = Url::parse(url).map_err(|error| {
        AsterError::validation_error(format!("invalid WOPI action URL: {error}"))
    })?;
    parsed.query_pairs_mut().append_pair("WOPISrc", wopi_src);
    Ok(parsed.to_string())
}

fn build_public_wopi_src(state: &AppState, file_id: i64) -> Result<String> {
    let Some(base) = site_url::public_site_url(&state.runtime_config) else {
        return Err(AsterError::validation_error(
            "public_site_url is required for WOPI integration",
        ));
    };

    Ok(format!("{base}/api/v1/wopi/files/{file_id}"))
}

async fn resolve_access_token(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<ResolvedWopiAccess> {
    let token_hash = access_token_hash(access_token);
    let session = wopi_session_repo::find_by_token_hash(&state.db, &token_hash)
        .await?
        .ok_or_else(|| AsterError::auth_token_invalid("WOPI access token not found or expired"))?;
    let payload = payload_from_session(&session)?;
    let expires_at = session.expires_at;
    if expires_at < Utc::now() {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(AsterError::auth_token_expired("WOPI access token expired"));
    }
    if payload.file_id != file_id {
        return Err(AsterError::file_not_found(format!(
            "WOPI token does not match file #{file_id}",
        )));
    }
    let auth_snapshot = auth_service::get_auth_snapshot(state, payload.actor_user_id).await?;
    if !auth_snapshot.status.is_active() {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(AsterError::auth_forbidden("account is disabled"));
    }
    if auth_snapshot.session_version != payload.session_version {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(AsterError::auth_token_invalid("WOPI session revoked"));
    }
    let Some(app) = preview_app_service::get_public_preview_apps(state)
        .apps
        .into_iter()
        .find(|candidate| candidate.key == payload.app_key)
    else {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(AsterError::auth_forbidden(
            "WOPI app is no longer available",
        ));
    };
    if !app.enabled {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(AsterError::auth_forbidden("WOPI app is disabled"));
    }
    if let Err(error) = parse_wopi_app_config(&app) {
        wopi_session_repo::delete_by_id(&state.db, session.id).await?;
        return Err(error);
    }
    ensure_request_source_allowed(&app, request_source)?;

    let file =
        workspace_storage_service::verify_file_access(state, scope_from_payload(&payload), file_id)
            .await?;

    Ok(ResolvedWopiAccess { file, payload })
}

async fn ensure_wopi_lock_matches(
    state: &AppState,
    payload: &WopiAccessTokenPayload,
    file_id: i64,
    requested_lock: Option<&str>,
) -> Result<Option<WopiConflict>> {
    let Some(active_lock) = load_active_lock(state, file_id).await? else {
        return Ok(None);
    };

    let Some(lock_payload) = active_lock.payload else {
        return Ok(Some(WopiConflict {
            current_lock: None,
            reason: "file is locked outside WOPI".to_string(),
        }));
    };

    let requested_lock = requested_lock
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AsterError::validation_error("X-WOPI-Lock header is required"))?;

    if lock_payload.app_key == payload.app_key && lock_payload.lock == requested_lock {
        return Ok(None);
    }

    Ok(Some(WopiConflict {
        current_lock: Some(lock_payload.lock),
        reason: "WOPI lock mismatch".to_string(),
    }))
}

async fn load_active_lock(state: &AppState, file_id: i64) -> Result<Option<ActiveWopiLock>> {
    let Some(lock) = lock_repo::find_by_entity(&state.db, EntityType::File, file_id).await? else {
        return Ok(None);
    };

    if let Some(timeout_at) = lock.timeout_at
        && timeout_at < Utc::now()
    {
        lock_repo::delete_by_id(&state.db, lock.id).await?;
        lock_service::set_entity_locked(&state.db, EntityType::File, file_id, false).await?;
        return Ok(None);
    }

    Ok(Some(ActiveWopiLock {
        payload: parse_wopi_lock_payload(lock.owner_info.as_deref()),
        lock,
    }))
}

async fn create_wopi_lock(
    state: &AppState,
    payload: &WopiAccessTokenPayload,
    file: &file::Model,
    requested_lock: &str,
) -> Result<()> {
    let path = lock_service::resolve_entity_path(&state.db, EntityType::File, file.id).await?;
    let now = Utc::now();
    let timeout_at = now + Duration::seconds(wopi::lock_ttl_secs(&state.runtime_config));
    let owner_info = serde_json::to_string(&WopiLockPayload {
        kind: "wopi".to_string(),
        app_key: payload.app_key.clone(),
        lock: requested_lock.to_string(),
    })
    .map_err(|_| AsterError::internal_error("failed to encode WOPI lock payload"))?;

    let model = resource_lock::ActiveModel {
        token: Set(format!("wopi:{}", uuid::Uuid::new_v4())),
        entity_type: Set(EntityType::File),
        entity_id: Set(file.id),
        path: Set(path),
        owner_id: Set(Some(payload.actor_user_id)),
        owner_info: Set(Some(owner_info)),
        timeout_at: Set(Some(timeout_at)),
        shared: Set(false),
        deep: Set(false),
        created_at: Set(now),
        ..Default::default()
    };

    lock_repo::create(&state.db, model).await?;
    lock_service::set_entity_locked(&state.db, EntityType::File, file.id, true).await?;
    Ok(())
}

async fn refresh_lock_model(state: &AppState, lock: resource_lock::Model) -> Result<()> {
    let mut active: resource_lock::ActiveModel = lock.into();
    active.timeout_at = Set(Some(
        Utc::now() + Duration::seconds(wopi::lock_ttl_secs(&state.runtime_config)),
    ));
    active.update(&state.db).await.map_err(AsterError::from)?;
    Ok(())
}

fn discovery_cache_ttl(runtime_config: &crate::config::RuntimeConfig) -> Duration {
    let ttl_secs = wopi::discovery_cache_ttl_secs(runtime_config);
    Duration::seconds(i64::try_from(ttl_secs).unwrap_or(i64::MAX))
}

fn parse_wopi_lock_payload(raw: Option<&str>) -> Option<WopiLockPayload> {
    let raw = raw?;
    let payload = serde_json::from_str::<WopiLockPayload>(raw).ok()?;
    (payload.kind == "wopi").then_some(payload)
}

fn scope_from_payload(payload: &WopiAccessTokenPayload) -> WorkspaceStorageScope {
    match payload.team_id {
        Some(team_id) => WorkspaceStorageScope::Team {
            team_id,
            actor_user_id: payload.actor_user_id,
        },
        None => WorkspaceStorageScope::Personal {
            user_id: payload.actor_user_id,
        },
    }
}

async fn create_access_token_session(
    state: &AppState,
    payload: &WopiAccessTokenPayload,
) -> Result<String> {
    let token = format!("wopi_{}", crate::utils::id::new_short_token());
    let token_hash = access_token_hash(&token);
    let expires_at = DateTime::from_timestamp(payload.exp, 0)
        .ok_or_else(|| AsterError::internal_error("invalid WOPI access token expiry"))?;
    let now = Utc::now();
    wopi_session_repo::create(
        &state.db,
        wopi_session::ActiveModel {
            token_hash: Set(token_hash),
            actor_user_id: Set(payload.actor_user_id),
            session_version: Set(payload.session_version),
            team_id: Set(payload.team_id),
            file_id: Set(payload.file_id),
            app_key: Set(payload.app_key.clone()),
            expires_at: Set(expires_at),
            created_at: Set(now),
            ..Default::default()
        },
    )
    .await?;
    Ok(token)
}

fn access_token_hash(token: &str) -> String {
    crate::utils::hash::sha256_hex(token.as_bytes())
}

fn payload_from_session(session: &wopi_session::Model) -> Result<WopiAccessTokenPayload> {
    Ok(WopiAccessTokenPayload {
        actor_user_id: session.actor_user_id,
        session_version: session.session_version,
        team_id: session.team_id,
        file_id: session.file_id,
        app_key: session.app_key.clone(),
        exp: session.expires_at.timestamp(),
    })
}

pub async fn cleanup_expired(state: &AppState) -> Result<u64> {
    wopi_session_repo::delete_expired(&state.db).await
}

fn file_extension(file_name: &str) -> Option<String> {
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim().to_ascii_lowercase())
        .filter(|ext| !ext.is_empty())
}

fn normalize_wopi_lock_value(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(
            "X-WOPI-Lock header must not be empty",
        ));
    }
    Ok(trimmed.to_string())
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

fn trusted_origins_for_app(app: &preview_app_service::PublicPreviewAppDefinition) -> Vec<String> {
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

fn ensure_request_source_allowed(
    app: &preview_app_service::PublicPreviewAppDefinition,
    request_source: WopiRequestSource<'_>,
) -> Result<()> {
    let trusted_origins = trusted_origins_for_app(app);
    if trusted_origins.is_empty() {
        return Ok(());
    }

    if let Some(origin) = request_source
        .origin
        .filter(|value| !value.trim().is_empty())
        .map(|value| cors::normalize_origin(value, false))
        .transpose()
        .map_err(|_| AsterError::validation_error("invalid Origin header"))?
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

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WopiCheckFileInfo, WopiRequestSource, access_token_hash, append_wopi_src,
        ensure_request_source_allowed, parse_discovery_xml, trusted_origins_for_app,
    };
    use crate::services::preview_app_service::{
        PreviewAppProvider, PreviewOpenMode, PublicPreviewAppConfig, PublicPreviewAppDefinition,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn test_wopi_app() -> PublicPreviewAppDefinition {
        PublicPreviewAppDefinition {
            key: "onlyoffice".to_string(),
            provider: PreviewAppProvider::Wopi,
            icon: "/icon.svg".to_string(),
            enabled: true,
            label_i18n_key: None,
            labels: BTreeMap::new(),
            config: PublicPreviewAppConfig {
                mode: Some(PreviewOpenMode::Iframe),
                action_url: Some(
                    "http://localhost:8080/hosting/wopi/word/edit?WOPISrc={{wopi_src}}".to_string(),
                ),
                discovery_url: Some("http://localhost:8080/hosting/discovery".to_string()),
                allowed_origins: vec!["http://127.0.0.1:8080".to_string()],
                ..Default::default()
            },
        }
    }

    #[test]
    fn append_wopi_src_adds_query_parameter() {
        let url = append_wopi_src(
            "https://office.example.com/hosting/wopi/word/edit?lang=zh-CN",
            "https://drive.example.com/api/v1/wopi/files/7",
        )
        .unwrap();
        assert!(url.contains("lang=zh-CN"));
        assert!(
            url.contains("WOPISrc=https%3A%2F%2Fdrive.example.com%2Fapi%2Fv1%2Fwopi%2Ffiles%2F7")
        );
    }

    #[test]
    fn parse_discovery_xml_extracts_named_actions() {
        let discovery = parse_discovery_xml(
            r#"
            <wopi-discovery>
              <net-zone name="external-http">
                <app name="application/vnd.openxmlformats-officedocument.wordprocessingml.document">
                  <action name="edit" ext="docx" urlsrc="https://office.example.com/word/edit?" />
                  <action name="view" ext="docx" urlsrc="https://office.example.com/word/view?" />
                </app>
              </net-zone>
            </wopi-discovery>
            "#,
        )
        .unwrap();

        assert_eq!(
            discovery
                .find_action_url(
                    "edit",
                    Some("docx"),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                )
                .as_deref(),
            Some("https://office.example.com/word/edit?")
        );
    }

    #[test]
    fn access_token_hash_is_stable_sha256_hex() {
        assert_eq!(
            access_token_hash("wopi_abc123"),
            crate::utils::hash::sha256_hex(b"wopi_abc123")
        );
    }

    #[test]
    fn trusted_origins_merge_explicit_and_derived_origins() {
        let origins = trusted_origins_for_app(&test_wopi_app());
        assert!(
            origins
                .iter()
                .any(|origin| origin == "http://localhost:8080")
        );
        assert!(
            origins
                .iter()
                .any(|origin| origin == "http://127.0.0.1:8080")
        );
    }

    #[test]
    fn request_source_check_accepts_matching_origin_or_missing_headers() {
        let app = test_wopi_app();

        ensure_request_source_allowed(
            &app,
            WopiRequestSource {
                origin: Some("http://localhost:8080"),
                referer: None,
            },
        )
        .unwrap();

        ensure_request_source_allowed(
            &app,
            WopiRequestSource {
                origin: None,
                referer: Some("http://localhost:8080/hosting/wopi/word/edit"),
            },
        )
        .unwrap();

        ensure_request_source_allowed(
            &app,
            WopiRequestSource {
                origin: None,
                referer: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn request_source_check_rejects_untrusted_origin() {
        let err = ensure_request_source_allowed(
            &test_wopi_app(),
            WopiRequestSource {
                origin: Some("https://evil.example.com"),
                referer: None,
            },
        )
        .unwrap_err();

        assert!(err.message().contains("untrusted WOPI request origin"));
    }

    #[test]
    fn check_file_info_serializes_user_can_not_write_relative() {
        let info = WopiCheckFileInfo {
            base_file_name: "doc.docx".to_string(),
            owner_id: "1".to_string(),
            size: 123,
            user_id: "2".to_string(),
            user_can_not_write_relative: true,
            user_can_rename: false,
            user_can_write: true,
            read_only: false,
            supports_get_lock: false,
            supports_locks: true,
            supports_rename: false,
            supports_update: true,
            version: "hash".to_string(),
        };

        let payload = serde_json::to_value(info).unwrap();
        assert_eq!(payload["UserCanNotWriteRelative"], json!(true));
    }
}
