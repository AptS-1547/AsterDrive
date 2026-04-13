use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use chrono::{DateTime, Duration, Utc};
use moka::future::Cache;
use reqwest::Url;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;
use xmltree::{Element, XMLNode};

use crate::config::{cors, site_url, wopi};
use crate::db::repository::{file_repo, lock_repo, wopi_session_repo};
use crate::entities::{file, resource_lock, wopi_session};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    auth_service, file_service, lock_service, preview_app_service, profile_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::{EntityType, NullablePatch};

static DISCOVERY_CACHE: LazyLock<Cache<String, CachedWopiDiscovery>> =
    LazyLock::new(|| Cache::builder().max_capacity(128).build());

static DISCOVERY_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(StdDuration::from_secs(5))
        .build()
        .expect("wopi discovery client should initialize")
});

const MAX_WOPI_LOCK_LEN: usize = 1024;
const MAX_WOPI_USER_INFO_LEN: usize = 1024;
const WOPI_FILE_NAME_MAX_LEN: i32 = 255;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct WopiLaunchSession {
    pub access_token: String,
    /// WOPI access token expiry time as a Unix timestamp in milliseconds.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name_max_length: Option<i32>,
    pub owner_id: String,
    pub size: i64,
    pub user_id: String,
    pub user_can_not_write_relative: bool,
    pub user_can_rename: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_info: Option<String>,
    pub user_can_write: bool,
    pub read_only: bool,
    pub supports_get_lock: bool,
    pub supports_locks: bool,
    pub supports_rename: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_user_info: Option<bool>,
    pub supports_update: bool,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct WopiConflict {
    pub current_lock: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WopiPutRelativeResponse {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WopiRenameFileResponse {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct WopiPutRelativeConflict {
    pub current_lock: Option<String>,
    pub reason: String,
    pub valid_target: Option<String>,
}

#[derive(Debug, Clone)]
pub enum WopiPutFileResult {
    Success { item_version: String },
    Conflict(WopiConflict),
}

#[derive(Debug, Clone)]
pub enum WopiPutRelativeResult {
    Success(WopiPutRelativeResponse),
    Conflict(WopiPutRelativeConflict),
}

#[derive(Debug, Clone)]
pub enum WopiGetLockResult {
    Success { current_lock: String },
    Conflict(WopiConflict),
}

#[derive(Debug, Clone)]
pub enum WopiLockOperationResult {
    Success,
    Conflict(WopiConflict),
}

#[derive(Debug, Clone)]
pub enum WopiRenameFileResult {
    Success(WopiRenameFileResponse),
    Conflict(WopiConflict),
    InvalidName { reason: String },
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
enum PutRelativeTargetMode {
    Suggested(String),
    Relative {
        target_name: String,
        overwrite: bool,
    },
}

#[derive(Debug, Clone)]
struct ParsedPutRelativeRequest {
    target_mode: PutRelativeTargetMode,
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
struct WopiDiscovery {
    actions: Vec<WopiDiscoveryAction>,
}

#[derive(Debug, Clone)]
struct CachedWopiDiscovery {
    discovery: WopiDiscovery,
    cached_at: DateTime<Utc>,
}

const DISCOVERY_ACTION_PRIORITY: &[&str] = &[
    "embededit",
    "edit",
    "mobileedit",
    "embedview",
    "view",
    "mobileview",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredWopiPreviewApp {
    pub action: String,
    pub extensions: Vec<String>,
    pub icon_url: Option<String>,
    pub key_suffix: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WopiRequestSource<'a> {
    pub origin: Option<&'a str>,
    pub referer: Option<&'a str>,
}

pub struct WopiPutRelativeRequest<'a> {
    pub file_id: i64,
    pub access_token: &'a str,
    pub body: actix_web::web::Bytes,
    pub suggested_target: Option<&'a str>,
    pub relative_target: Option<&'a str>,
    pub overwrite_relative_target: Option<&'a str>,
    pub size_header: Option<&'a str>,
    pub request_source: WopiRequestSource<'a>,
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
    let user_info =
        profile_service::get_wopi_user_info(state, resolved.payload.actor_user_id).await?;

    Ok(WopiCheckFileInfo {
        base_file_name: resolved.file.name.clone(),
        file_name_max_length: Some(WOPI_FILE_NAME_MAX_LEN),
        owner_id: resolved.file.user_id.to_string(),
        size: resolved.file.size,
        user_id: resolved.payload.actor_user_id.to_string(),
        user_can_not_write_relative: false,
        user_can_rename: true,
        user_info,
        user_can_write: true,
        read_only: false,
        supports_get_lock: true,
        supports_locks: true,
        supports_rename: true,
        supports_user_info: Some(true),
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

pub async fn put_relative_file(
    state: &AppState,
    req: WopiPutRelativeRequest<'_>,
) -> Result<WopiPutRelativeResult> {
    let WopiPutRelativeRequest {
        file_id,
        access_token,
        body,
        suggested_target,
        relative_target,
        overwrite_relative_target,
        size_header,
        request_source,
    } = req;
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let request = parse_put_relative_request(
        &resolved.file.name,
        suggested_target,
        relative_target,
        overwrite_relative_target,
        size_header,
        body.len(),
    )?;
    let scope = scope_from_payload(&resolved.payload);

    let target_file = match request.target_mode {
        PutRelativeTargetMode::Suggested(target_name) => {
            store_relative_target_from_bytes(
                state,
                scope,
                resolved.file.folder_id,
                &target_name,
                None,
                &body,
                false,
            )
            .await?
        }
        PutRelativeTargetMode::Relative {
            target_name,
            overwrite,
        } => {
            let existing =
                find_file_by_name_in_scope(&state.db, scope, resolved.file.folder_id, &target_name)
                    .await?;

            let existing = match existing {
                Some(existing) => existing,
                None => {
                    store_relative_target_from_bytes(
                        state,
                        scope,
                        resolved.file.folder_id,
                        &target_name,
                        None,
                        &body,
                        true,
                    )
                    .await?
                }
            };

            if existing.id == resolved.file.id {
                return Err(AsterError::validation_error(
                    "PUT_RELATIVE target must differ from source file",
                ));
            }

            if !overwrite {
                let valid_target = encode_wopi_filename(
                    &suggest_available_relative_target(
                        state,
                        scope,
                        resolved.file.folder_id,
                        &target_name,
                    )
                    .await?,
                );
                return Ok(WopiPutRelativeResult::Conflict(WopiPutRelativeConflict {
                    current_lock: Some(String::new()),
                    reason: "target file already exists".to_string(),
                    valid_target: Some(valid_target),
                }));
            }

            if let Some(active_lock) = load_active_lock(state, existing.id).await? {
                return Ok(WopiPutRelativeResult::Conflict(WopiPutRelativeConflict {
                    current_lock: Some(active_wopi_lock_value(&active_lock).unwrap_or_default()),
                    reason: "target file is locked".to_string(),
                    valid_target: None,
                }));
            }

            store_relative_target_from_bytes(
                state,
                scope,
                resolved.file.folder_id,
                &target_name,
                Some(existing.id),
                &body,
                true,
            )
            .await?
        }
    };

    let response =
        build_put_relative_response(state, &resolved.payload, &target_file.name, target_file.id)
            .await?;
    Ok(WopiPutRelativeResult::Success(response))
}

pub async fn get_lock(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiGetLockResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiGetLockResult::Success {
            current_lock: String::new(),
        });
    };

    match active_lock.payload {
        Some(payload) => Ok(WopiGetLockResult::Success {
            current_lock: payload.lock,
        }),
        None => Ok(WopiGetLockResult::Conflict(WopiConflict {
            current_lock: Some(String::new()),
            reason: "file is locked outside WOPI".to_string(),
        })),
    }
}

pub async fn rename_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_name: Option<&str>,
    requested_lock: Option<&str>,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiRenameFileResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    if let Some(conflict) =
        ensure_wopi_lock_matches(state, &resolved.payload, resolved.file.id, requested_lock).await?
    {
        return Ok(WopiRenameFileResult::Conflict(conflict));
    }

    let requested_name =
        match normalize_requested_rename_target(&resolved.file.name, requested_name) {
            Ok(name) => name,
            Err(reason) => return Ok(WopiRenameFileResult::InvalidName { reason }),
        };
    let scope = scope_from_payload(&resolved.payload);
    let mut final_name = resolve_available_rename_target(
        state,
        scope,
        resolved.file.folder_id,
        resolved.file.id,
        &requested_name,
    )
    .await?;

    let updated = match file_service::update_in_scope(
        state,
        scope,
        resolved.file.id,
        Some(final_name.clone()),
        NullablePatch::Absent,
    )
    .await
    {
        Ok(updated) => updated,
        Err(err) if file_repo::is_duplicate_name_error(&err, &final_name) => {
            final_name = suggest_available_relative_target(
                state,
                scope,
                resolved.file.folder_id,
                &final_name,
            )
            .await?;
            file_service::update_in_scope(
                state,
                scope,
                resolved.file.id,
                Some(final_name),
                NullablePatch::Absent,
            )
            .await?
        }
        Err(err) => return Err(err),
    };

    Ok(WopiRenameFileResult::Success(WopiRenameFileResponse {
        name: response_name_for_rename(&resolved.file.name, &updated.name).to_string(),
    }))
}

pub async fn put_user_info(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    body: actix_web::web::Bytes,
    request_source: WopiRequestSource<'_>,
) -> Result<()> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let user_info = normalize_wopi_user_info(&body)?;
    profile_service::update_wopi_user_info(state, resolved.payload.actor_user_id, user_info).await
}

pub async fn lock_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let lock_value = normalize_wopi_lock_header("X-WOPI-Lock", requested_lock)?;
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
            current_lock: Some(String::new()),
            reason: "file is locked outside WOPI".to_string(),
        }));
    }

    create_wopi_lock(state, &resolved.payload, &resolved.file, &lock_value).await?;
    Ok(WopiLockOperationResult::Success)
}

pub async fn unlock_and_relock_file(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    old_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let new_lock = normalize_wopi_lock_header("X-WOPI-Lock", requested_lock)?;
    let old_lock = normalize_wopi_lock_header("X-WOPI-OldLock", old_lock)?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(String::new()),
            reason: "file is not locked".to_string(),
        }));
    };

    match active_lock.payload {
        Some(payload)
            if payload.app_key == resolved.payload.app_key && payload.lock == old_lock =>
        {
            replace_wopi_lock_model(state, active_lock.lock, &resolved.payload, &new_lock).await?;
            Ok(WopiLockOperationResult::Success)
        }
        Some(payload) => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(payload.lock),
            reason: "WOPI lock mismatch".to_string(),
        })),
        None => Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(String::new()),
            reason: "file is locked outside WOPI".to_string(),
        })),
    }
}

pub async fn refresh_lock(
    state: &AppState,
    file_id: i64,
    access_token: &str,
    requested_lock: &str,
    request_source: WopiRequestSource<'_>,
) -> Result<WopiLockOperationResult> {
    let resolved = resolve_access_token(state, file_id, access_token, request_source).await?;
    let lock_value = normalize_wopi_lock_header("X-WOPI-Lock", requested_lock)?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(String::new()),
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
            current_lock: Some(String::new()),
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
    let lock_value = normalize_wopi_lock_header("X-WOPI-Lock", requested_lock)?;
    let Some(active_lock) = load_active_lock(state, resolved.file.id).await? else {
        return Ok(WopiLockOperationResult::Conflict(WopiConflict {
            current_lock: Some(String::new()),
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
            current_lock: Some(String::new()),
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

fn normalize_wopi_user_info(body: &actix_web::web::Bytes) -> Result<String> {
    let user_info = std::str::from_utf8(body)
        .map_err(|_| AsterError::validation_error("PUT_USER_INFO body must be valid UTF-8"))?;
    if !user_info.is_ascii() {
        return Err(AsterError::validation_error(
            "PUT_USER_INFO body must contain ASCII characters only",
        ));
    }
    if user_info.len() > MAX_WOPI_USER_INFO_LEN {
        return Err(AsterError::validation_error(format!(
            "PUT_USER_INFO body must be {MAX_WOPI_USER_INFO_LEN} bytes or fewer"
        )));
    }
    Ok(user_info.to_string())
}

fn parse_put_relative_request(
    source_file_name: &str,
    suggested_target: Option<&str>,
    relative_target: Option<&str>,
    overwrite_relative_target: Option<&str>,
    size_header: Option<&str>,
    body_len: usize,
) -> Result<ParsedPutRelativeRequest> {
    if let Some(size_header) = size_header {
        let declared_size = size_header.parse::<usize>().map_err(|_| {
            AsterError::validation_error("X-WOPI-Size header must be a non-negative integer")
        })?;
        if declared_size != body_len {
            return Err(AsterError::validation_error(
                "X-WOPI-Size header does not match request body length",
            ));
        }
    }

    match (suggested_target, relative_target) {
        (Some(_), Some(_)) => Err(AsterError::validation_error(
            "PUT_RELATIVE requires exactly one of X-WOPI-SuggestedTarget or X-WOPI-RelativeTarget",
        )),
        (None, None) => Err(AsterError::validation_error(
            "PUT_RELATIVE requires X-WOPI-SuggestedTarget or X-WOPI-RelativeTarget",
        )),
        (Some(suggested_target), None) => {
            let decoded = decode_wopi_filename(suggested_target)?;
            let target_name = normalize_suggested_target_name(source_file_name, &decoded);
            Ok(ParsedPutRelativeRequest {
                target_mode: PutRelativeTargetMode::Suggested(target_name),
            })
        }
        (None, Some(relative_target)) => {
            let decoded = decode_wopi_filename(relative_target)?;
            let overwrite = parse_overwrite_relative_target(overwrite_relative_target)?;
            Ok(ParsedPutRelativeRequest {
                target_mode: PutRelativeTargetMode::Relative {
                    target_name: normalize_relative_target_name(&decoded)?,
                    overwrite,
                },
            })
        }
    }
}

fn parse_overwrite_relative_target(raw: Option<&str>) -> Result<bool> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(false);
    };

    if raw.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if raw.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(AsterError::validation_error(
            "X-WOPI-OverwriteRelativeTarget must be true or false",
        ))
    }
}

fn normalize_relative_target_name(value: &str) -> Result<String> {
    crate::utils::validate_name(value)?;
    Ok(value.to_string())
}

fn normalize_requested_rename_target(
    source_file_name: &str,
    requested_name: Option<&str>,
) -> std::result::Result<String, String> {
    let Some(requested_name) = requested_name else {
        return Err("X-WOPI-RequestedName header is required".to_string());
    };

    let decoded = decode_wopi_filename(requested_name).map_err(|err| err.message().to_string())?;
    match build_requested_rename_filename(source_file_name, &decoded) {
        Ok(name) => Ok(name),
        Err(_) => sanitize_requested_rename_name(source_file_name, &decoded)
            .ok_or_else(|| "invalid requested file name".to_string()),
    }
}

fn build_requested_rename_filename(source_file_name: &str, requested_name: &str) -> Result<String> {
    let requested_name = requested_name.trim();
    if requested_name.is_empty() {
        return Err(AsterError::validation_error(
            "requested file name cannot be empty",
        ));
    }

    let full_name = rename_target_name(source_file_name, requested_name);
    crate::utils::validate_name(&full_name)?;
    Ok(full_name)
}

fn sanitize_requested_rename_name(source_file_name: &str, requested_name: &str) -> Option<String> {
    let mut sanitized: String = requested_name
        .chars()
        .filter(|ch| !is_forbidden_file_name_char(*ch))
        .collect();
    sanitized = sanitized.trim().trim_end_matches('.').to_string();
    truncate_utf8_to_len(&mut sanitized, max_requested_rename_len(source_file_name));
    sanitized = sanitized.trim().trim_end_matches('.').to_string();

    (!sanitized.is_empty())
        .then(|| build_requested_rename_filename(source_file_name, &sanitized).ok())
        .flatten()
}

fn rename_target_name(source_file_name: &str, requested_name: &str) -> String {
    match file_extension(source_file_name) {
        Some(ext) => format!("{requested_name}.{ext}"),
        None => requested_name.to_string(),
    }
}

fn max_requested_rename_len(source_file_name: &str) -> usize {
    usize::try_from(WOPI_FILE_NAME_MAX_LEN).unwrap_or(255)
        - file_extension(source_file_name).map_or(0, |ext| ext.len() + 1)
}

fn response_name_for_rename<'a>(source_file_name: &str, renamed_file_name: &'a str) -> &'a str {
    if file_extension(source_file_name).is_some() {
        source_file_stem(renamed_file_name)
    } else {
        renamed_file_name
    }
}

fn truncate_utf8_to_len(value: &mut String, max_len: usize) {
    if value.len() <= max_len {
        return;
    }

    let mut truncate_at = 0;
    for (index, ch) in value.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_len {
            break;
        }
        truncate_at = next;
    }

    value.truncate(truncate_at);
}

fn normalize_suggested_target_name(source_file_name: &str, value: &str) -> String {
    let candidate = if value.starts_with('.') {
        format!("{}{}", source_file_stem(source_file_name), value)
    } else {
        value.to_string()
    };

    sanitize_suggested_target_name(&candidate, source_file_name)
}

fn sanitize_suggested_target_name(candidate: &str, fallback: &str) -> String {
    let mut sanitized: String = candidate
        .chars()
        .filter(|ch| !is_forbidden_file_name_char(*ch))
        .collect();
    sanitized = sanitized.trim().trim_end_matches('.').to_string();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return fallback.to_string();
    }

    if sanitized.len() > 255 {
        truncate_utf8_to_len(&mut sanitized, 255);
        sanitized = sanitized.trim().trim_end_matches('.').to_string();
    }

    if crate::utils::validate_name(&sanitized).is_ok() {
        sanitized
    } else {
        fallback.to_string()
    }
}

fn source_file_stem(value: &str) -> &str {
    match value.rfind('.') {
        Some(dot) if dot > 0 => &value[..dot],
        _ => value,
    }
}

fn is_forbidden_file_name_char(ch: char) -> bool {
    matches!(
        ch,
        '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
    ) || ch.is_ascii_control()
}

async fn find_file_by_name_in_scope<C: ConnectionTrait>(
    db: &C,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<Option<file::Model>> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::find_by_name_in_folder(db, user_id, folder_id, name).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::find_by_name_in_team_folder(db, team_id, folder_id, name).await
        }
    }
}

async fn suggest_available_relative_target(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    name: &str,
) -> Result<String> {
    match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            file_repo::resolve_unique_filename(&state.db, user_id, folder_id, name).await
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            file_repo::resolve_unique_team_filename(&state.db, team_id, folder_id, name).await
        }
    }
}

async fn resolve_available_rename_target(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    current_file_id: i64,
    requested_name: &str,
) -> Result<String> {
    let existing = find_file_by_name_in_scope(&state.db, scope, folder_id, requested_name).await?;
    if match existing.as_ref() {
        None => true,
        Some(file) => file.id == current_file_id,
    } {
        return Ok(requested_name.to_string());
    }

    suggest_available_relative_target(state, scope, folder_id, requested_name).await
}

async fn store_relative_target_from_bytes(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    filename: &str,
    existing_file_id: Option<i64>,
    body: &actix_web::web::Bytes,
    exact_name: bool,
) -> Result<file::Model> {
    let size = i64::try_from(body.len())
        .map_err(|_| AsterError::validation_error("PUT_RELATIVE body is too large"))?;
    let resolved_policy =
        workspace_storage_service::resolve_policy_for_size(state, scope, folder_id, size).await?;

    if resolved_policy.driver_type == crate::types::DriverType::Local {
        let should_dedup = workspace_storage_service::local_content_dedup_enabled(&resolved_policy);
        let staging_token = format!("{}.upload", uuid::Uuid::new_v4());
        let staging_path =
            crate::storage::local::upload_staging_path(&resolved_policy, &staging_token);
        if let Some(parent) = staging_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_aster_err(AsterError::storage_driver_error)?;
        }
        tokio::fs::write(&staging_path, body)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        let precomputed_hash = should_dedup.then(|| {
            let mut hasher = Sha256::new();
            hasher.update(body);
            crate::utils::hash::sha256_digest_to_hex(&hasher.finalize())
        });
        let staging_path = staging_path.to_string_lossy().into_owned();
        let result = if exact_name {
            workspace_storage_service::store_from_temp_exact_name_with_hints(
                state,
                scope,
                folder_id,
                filename,
                &staging_path,
                size,
                existing_file_id,
                existing_file_id.is_some(),
                Some(resolved_policy),
                precomputed_hash.as_deref(),
            )
            .await
        } else {
            workspace_storage_service::store_from_temp_with_hints(
                state,
                scope,
                folder_id,
                filename,
                &staging_path,
                size,
                existing_file_id,
                existing_file_id.is_some(),
                Some(resolved_policy),
                precomputed_hash.as_deref(),
            )
            .await
        };
        crate::utils::cleanup_temp_file(&staging_path).await;
        result
    } else {
        let temp_dir = &state.config.server.temp_dir;
        let temp_path =
            crate::utils::paths::temp_file_path(temp_dir, &uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(temp_dir)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;
        tokio::fs::write(&temp_path, body)
            .await
            .map_aster_err(AsterError::storage_driver_error)?;

        let result = if exact_name {
            workspace_storage_service::store_from_temp_exact_name_with_hints(
                state,
                scope,
                folder_id,
                filename,
                &temp_path,
                size,
                existing_file_id,
                existing_file_id.is_some(),
                Some(resolved_policy),
                None,
            )
            .await
        } else {
            workspace_storage_service::store_from_temp_with_hints(
                state,
                scope,
                folder_id,
                filename,
                &temp_path,
                size,
                existing_file_id,
                existing_file_id.is_some(),
                Some(resolved_policy),
                None,
            )
            .await
        };
        crate::utils::cleanup_temp_file(&temp_path).await;
        result
    }
}

async fn build_put_relative_response(
    state: &AppState,
    payload: &WopiAccessTokenPayload,
    target_name: &str,
    target_file_id: i64,
) -> Result<WopiPutRelativeResponse> {
    let access_token = create_access_token_for_file(state, payload, target_file_id).await?;
    let url = format!(
        "{}?access_token={}",
        build_public_wopi_src(state, target_file_id)?,
        urlencoding::encode(&access_token)
    );

    Ok(WopiPutRelativeResponse {
        name: target_name.to_string(),
        url,
    })
}

async fn create_access_token_for_file(
    state: &AppState,
    payload: &WopiAccessTokenPayload,
    file_id: i64,
) -> Result<String> {
    let expires_at =
        Utc::now() + Duration::seconds(wopi::access_token_ttl_secs(&state.runtime_config));
    create_access_token_session(
        state,
        &WopiAccessTokenPayload {
            file_id,
            exp: expires_at.timestamp(),
            ..payload.clone()
        },
    )
    .await
}

fn active_wopi_lock_value(active_lock: &ActiveWopiLock) -> Option<String> {
    active_lock
        .payload
        .as_ref()
        .map(|payload| payload.lock.clone())
}

fn decode_wopi_filename(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AsterError::validation_error(
            "WOPI target header must not be empty",
        ));
    }

    let mut decoded = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '+' {
            decoded.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('-')) {
            chars.next();
            decoded.push('+');
            continue;
        }

        let mut shifted = String::new();
        while let Some(&next) = chars.peek() {
            if next == '-' {
                chars.next();
                break;
            }
            shifted.push(next);
            chars.next();
        }

        if shifted.is_empty() {
            return Err(AsterError::validation_error(
                "invalid UTF-7 sequence in WOPI target header",
            ));
        }

        let mut padded = shifted.clone();
        while !padded.len().is_multiple_of(4) {
            padded.push('=');
        }
        let bytes = STANDARD.decode(padded.as_bytes()).map_err(|_| {
            AsterError::validation_error("invalid UTF-7 base64 payload in WOPI target header")
        })?;
        if bytes.len() % 2 != 0 {
            return Err(AsterError::validation_error(
                "invalid UTF-7 payload length in WOPI target header",
            ));
        }

        let utf16 = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]));
        for ch in char::decode_utf16(utf16) {
            decoded.push(ch.map_err(|_| {
                AsterError::validation_error("invalid UTF-16 sequence in WOPI target header")
            })?);
        }
    }

    Ok(decoded)
}

fn encode_wopi_filename(value: &str) -> String {
    let mut encoded = String::new();
    let mut shifted = String::new();

    let flush_shifted = |encoded: &mut String, shifted: &mut String| {
        if shifted.is_empty() {
            return;
        }

        let mut utf16 = Vec::with_capacity(shifted.len() * 2);
        for unit in shifted.encode_utf16() {
            utf16.extend_from_slice(&unit.to_be_bytes());
        }
        encoded.push('+');
        encoded.push_str(&STANDARD_NO_PAD.encode(utf16));
        encoded.push('-');
        shifted.clear();
    };

    for ch in value.chars() {
        if ch == '+' {
            flush_shifted(&mut encoded, &mut shifted);
            encoded.push_str("+-");
        } else if is_direct_utf7_char(ch) {
            flush_shifted(&mut encoded, &mut shifted);
            encoded.push(ch);
        } else {
            shifted.push(ch);
        }
    }

    flush_shifted(&mut encoded, &mut shifted);
    encoded
}

fn is_direct_utf7_char(ch: char) -> bool {
    ch.is_ascii() && !ch.is_ascii_control()
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
    collect_discovery_actions(&root, None, None, &mut actions);
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

fn resolve_discovery_action_url(
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

fn build_discovered_preview_apps(discovery: &WopiDiscovery) -> Vec<DiscoveredWopiPreviewApp> {
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
            current_lock: Some(String::new()),
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
    let owner_info = encode_wopi_lock_payload(payload, requested_lock)?;

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

async fn replace_wopi_lock_model(
    state: &AppState,
    lock: resource_lock::Model,
    payload: &WopiAccessTokenPayload,
    requested_lock: &str,
) -> Result<()> {
    let mut active: resource_lock::ActiveModel = lock.into();
    active.owner_info = Set(Some(encode_wopi_lock_payload(payload, requested_lock)?));
    active.timeout_at = Set(Some(
        Utc::now() + Duration::seconds(wopi::lock_ttl_secs(&state.runtime_config)),
    ));
    active.update(&state.db).await.map_err(AsterError::from)?;
    Ok(())
}

fn encode_wopi_lock_payload(
    payload: &WopiAccessTokenPayload,
    requested_lock: &str,
) -> Result<String> {
    serde_json::to_string(&WopiLockPayload {
        kind: "wopi".to_string(),
        app_key: payload.app_key.clone(),
        lock: requested_lock.to_string(),
    })
    .map_err(|_| AsterError::internal_error("failed to encode WOPI lock payload"))
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

fn normalize_wopi_lock_header(header_name: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(format!(
            "{header_name} header must not be empty"
        )));
    }
    if !trimmed.is_ascii() {
        return Err(AsterError::validation_error(format!(
            "{header_name} header must contain ASCII characters only"
        )));
    }
    if trimmed.len() > MAX_WOPI_LOCK_LEN {
        return Err(AsterError::validation_error(format!(
            "{header_name} header must be {MAX_WOPI_LOCK_LEN} bytes or fewer"
        )));
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
        PutRelativeTargetMode, WopiCheckFileInfo, WopiRequestSource, access_token_hash,
        append_wopi_src, build_discovered_preview_apps, decode_wopi_filename, encode_wopi_filename,
        ensure_request_source_allowed, expand_action_url, parse_discovery_xml,
        parse_put_relative_request, resolve_discovery_action_url, trusted_origins_for_app,
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
            extensions: vec!["docx".to_string()],
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
    fn expand_action_url_resolves_discovery_placeholders() {
        let url = expand_action_url(
            "https://office.example.com/hosting/wopi/word/view?mobile=1&<ui=UI_LLCC&><rs=DC_LLCC&><wopisrc=WOPI_SOURCE&>",
            "https://drive.example.com/api/v1/wopi/files/7",
        )
        .unwrap();

        assert!(url.contains("mobile=1"));
        assert!(
            url.contains("wopisrc=https%3A%2F%2Fdrive.example.com%2Fapi%2Fv1%2Fwopi%2Ffiles%2F7")
        );
        assert!(!url.contains("<ui="));
        assert!(!url.contains("<wopisrc="));
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
    fn build_discovered_preview_apps_groups_actions_by_app_name() {
        let discovery = parse_discovery_xml(
            r#"
            <wopi-discovery>
              <net-zone name="external-http">
                <app name="Word" favIconUrl="https://office.example.com/word.ico">
                  <action name="view" ext="doc" urlsrc="https://office.example.com/word/view?" />
                  <action name="view" ext="docx" urlsrc="https://office.example.com/word/view?" />
                  <action name="edit" ext="docx" urlsrc="https://office.example.com/word/edit?" />
                </app>
                <app name="Excel" favIconUrl="https://office.example.com/excel.ico">
                  <action name="view" ext="xls" urlsrc="https://office.example.com/excel/view?" />
                  <action name="view" ext="xlsx" urlsrc="https://office.example.com/excel/view?" />
                </app>
                <app name="Pdf" favIconUrl="https://office.example.com/pdf.ico">
                  <action name="view" ext="pdf" urlsrc="https://office.example.com/pdf/view?" />
                </app>
              </net-zone>
            </wopi-discovery>
            "#,
        )
        .unwrap();

        let apps = build_discovered_preview_apps(&discovery);

        assert_eq!(apps.len(), 3);
        assert_eq!(
            apps[0],
            super::DiscoveredWopiPreviewApp {
                action: "edit".to_string(),
                extensions: vec!["doc".to_string(), "docx".to_string()],
                icon_url: Some("https://office.example.com/word.ico".to_string()),
                key_suffix: "word".to_string(),
                label: "Word".to_string(),
            }
        );
        assert_eq!(
            apps[1],
            super::DiscoveredWopiPreviewApp {
                action: "view".to_string(),
                extensions: vec!["xls".to_string(), "xlsx".to_string()],
                icon_url: Some("https://office.example.com/excel.ico".to_string()),
                key_suffix: "excel".to_string(),
                label: "Excel".to_string(),
            }
        );
        assert_eq!(
            apps[2],
            super::DiscoveredWopiPreviewApp {
                action: "view".to_string(),
                extensions: vec!["pdf".to_string()],
                icon_url: Some("https://office.example.com/pdf.ico".to_string()),
                key_suffix: "pdf".to_string(),
                label: "Pdf".to_string(),
            }
        );
    }

    #[test]
    fn resolve_discovery_action_url_prefers_editable_actions_for_legacy_view_configs() {
        let discovery = parse_discovery_xml(
            r#"
            <wopi-discovery>
              <net-zone name="external-http">
                <app name="Word">
                  <action name="view" ext="doc" urlsrc="https://office.example.com/word/view?" />
                  <action name="view" ext="docx" urlsrc="https://office.example.com/word/view?" />
                  <action name="edit" ext="docx" urlsrc="https://office.example.com/word/edit?" />
                </app>
              </net-zone>
            </wopi-discovery>
            "#,
        )
        .unwrap();

        assert_eq!(
            resolve_discovery_action_url(
                &discovery,
                "view",
                Some("docx"),
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )
            .as_deref(),
            Some("https://office.example.com/word/edit?")
        );
        assert_eq!(
            resolve_discovery_action_url(&discovery, "edit", Some("doc"), "application/msword",)
                .as_deref(),
            Some("https://office.example.com/word/view?")
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
        use crate::services::wopi_service::WOPI_FILE_NAME_MAX_LEN;

        let info = WopiCheckFileInfo {
            base_file_name: "doc.docx".to_string(),
            file_name_max_length: Some(WOPI_FILE_NAME_MAX_LEN),
            owner_id: "1".to_string(),
            size: 123,
            user_id: "2".to_string(),
            user_can_not_write_relative: false,
            user_can_rename: true,
            user_info: Some("pane-state".to_string()),
            user_can_write: true,
            read_only: false,
            supports_get_lock: true,
            supports_locks: true,
            supports_rename: true,
            supports_user_info: Some(true),
            supports_update: true,
            version: "hash".to_string(),
        };

        let payload = serde_json::to_value(info).unwrap();
        assert_eq!(payload["UserCanNotWriteRelative"], json!(false));
    }

    #[test]
    fn utf7_roundtrip_handles_non_ascii_targets() {
        let encoded = encode_wopi_filename("副本 文档.docx");
        let decoded = decode_wopi_filename(&encoded).unwrap();
        assert_eq!(decoded, "副本 文档.docx");
    }

    #[test]
    fn parse_put_relative_request_allows_extension_only_suggested_target() {
        let request =
            parse_put_relative_request("report 1.docx", Some(".docx"), None, None, Some("4"), 4)
                .unwrap();

        match request.target_mode {
            PutRelativeTargetMode::Suggested(name) => assert_eq!(name, "report 1.docx"),
            PutRelativeTargetMode::Relative { .. } => panic!("expected suggested target"),
        }
    }
}
