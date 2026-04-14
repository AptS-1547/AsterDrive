use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::cache::CacheExt;
use crate::config::site_url;
use crate::db::repository::file_repo;
use crate::entities::{file, share};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    direct_link_service, file_service, share_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

const PREVIEW_LINK_TTL_SECS: i64 = 5 * 60;
const PREVIEW_LINK_MAX_USES: u32 = 5;
const PREVIEW_LINK_CACHE_PREFIX: &str = "preview_link:";

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PreviewLinkInfo {
    pub path: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub expires_at: DateTime<Utc>,
    pub max_uses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PreviewSubject {
    File { file_id: i64 },
    ShareFile { share_token: String },
    ShareFolderFile { share_token: String, file_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PreviewTokenPayload {
    subject: PreviewSubject,
    exp: i64,
    max_uses: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PreviewUsageState {
    used: u32,
}

struct ReservedUse {
    cache_key: String,
    previous_used: u32,
    ttl_secs: u64,
}

enum ResolvedPreviewTarget {
    File {
        payload: PreviewTokenPayload,
        file: file::Model,
    },
    Shared {
        payload: PreviewTokenPayload,
        file: file::Model,
    },
}

pub(crate) async fn create_token_for_file_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
) -> Result<PreviewLinkInfo> {
    let file = workspace_storage_service::verify_file_access(state, scope, file_id).await?;
    let payload = build_payload(PreviewSubject::File { file_id: file.id });
    build_link_for_file(state, &file, &payload)
}

pub async fn create_token_for_shared_file(
    state: &AppState,
    share_token: &str,
) -> Result<PreviewLinkInfo> {
    let (share, file) = share_service::load_preview_shared_file(state, share_token).await?;
    let payload = build_payload(PreviewSubject::ShareFile {
        share_token: share.token.clone(),
    });
    build_link_for_shared_file(state, &share, &file, &payload)
}

pub async fn create_token_for_shared_folder_file(
    state: &AppState,
    share_token: &str,
    file_id: i64,
) -> Result<PreviewLinkInfo> {
    let (share, file) =
        share_service::load_preview_shared_folder_file(state, share_token, file_id).await?;
    let payload = build_payload(PreviewSubject::ShareFolderFile {
        share_token: share.token.clone(),
        file_id: file.id,
    });
    build_link_for_shared_file(state, &share, &file, &payload)
}

pub async fn download_file(
    state: &AppState,
    token: &str,
    requested_name: &str,
    if_none_match: Option<&str>,
) -> Result<actix_web::HttpResponse> {
    let resolved = resolve_token(state, token).await?;
    let (payload, file) = match &resolved {
        ResolvedPreviewTarget::File { payload, file } => (payload, file),
        ResolvedPreviewTarget::Shared { payload, file, .. } => (payload, file),
    };

    direct_link_service::validate_public_file_name(file, requested_name)?;

    let blob = file_repo::find_blob_by_id(&state.db, file.blob_id).await?;
    if let Some(if_none_match) = if_none_match
        && file_service::if_none_match_matches(if_none_match, &blob.hash)
    {
        return file_service::build_stream_response_with_disposition(
            state,
            file,
            &blob,
            file_service::DownloadDisposition::Inline,
            Some(if_none_match),
        )
        .await;
    }

    let reserved = reserve_usage(state, token, payload).await?;
    match file_service::build_stream_response_with_disposition(
        state,
        file,
        &blob,
        file_service::DownloadDisposition::Inline,
        None,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            rollback_usage(state, &reserved).await;
            Err(error)
        }
    }
}

fn build_payload(subject: PreviewSubject) -> PreviewTokenPayload {
    PreviewTokenPayload {
        subject,
        exp: (Utc::now() + Duration::seconds(PREVIEW_LINK_TTL_SECS)).timestamp(),
        max_uses: PREVIEW_LINK_MAX_USES,
    }
}

fn build_link_for_file(
    state: &AppState,
    file: &file::Model,
    payload: &PreviewTokenPayload,
) -> Result<PreviewLinkInfo> {
    let token = encode_file_token(file, payload, &state.config.auth.jwt_secret)?;
    Ok(PreviewLinkInfo {
        path: preview_path(&state.runtime_config, &token, &file.name),
        expires_at: decode_expiry(payload.exp)?,
        max_uses: payload.max_uses,
    })
}

fn build_link_for_shared_file(
    state: &AppState,
    share: &share::Model,
    file: &file::Model,
    payload: &PreviewTokenPayload,
) -> Result<PreviewLinkInfo> {
    let token = encode_shared_token(share, file, payload, &state.config.auth.jwt_secret)?;
    Ok(PreviewLinkInfo {
        path: preview_path(&state.runtime_config, &token, &file.name),
        expires_at: decode_expiry(payload.exp)?,
        max_uses: payload.max_uses,
    })
}

fn preview_path(
    runtime_config: &crate::config::RuntimeConfig,
    token: &str,
    file_name: &str,
) -> String {
    let path = format!("/pv/{token}/{}", urlencoding::encode(file_name));
    site_url::public_app_url_or_path(runtime_config, &path)
}

fn encode_file_token(
    file: &file::Model,
    payload: &PreviewTokenPayload,
    secret: &str,
) -> Result<String> {
    let payload_segment = encode_payload(payload)?;
    let signature = sign_file_payload(file, &payload_segment, secret);
    Ok(format!("{payload_segment}.{signature}"))
}

fn encode_shared_token(
    share: &share::Model,
    file: &file::Model,
    payload: &PreviewTokenPayload,
    secret: &str,
) -> Result<String> {
    let payload_segment = encode_payload(payload)?;
    let signature = sign_shared_payload(share, file, &payload_segment, secret);
    Ok(format!("{payload_segment}.{signature}"))
}

fn encode_payload(payload: &PreviewTokenPayload) -> Result<String> {
    let bytes = serde_json::to_vec(payload)
        .map_aster_err_ctx("failed to encode preview token", AsterError::internal_error)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

async fn resolve_token(state: &AppState, token: &str) -> Result<ResolvedPreviewTarget> {
    let (payload_segment, signature) = split_token(token)?;
    let payload = decode_payload(payload_segment)?;
    let expires_at = decode_expiry(payload.exp)?;
    if expires_at < Utc::now() {
        return Err(AsterError::share_expired("preview link expired"));
    }

    match &payload.subject {
        PreviewSubject::File { file_id } => {
            let file = direct_link_service::load_public_file(state, *file_id).await?;
            let expected = sign_file_payload(&file, payload_segment, &state.config.auth.jwt_secret);
            if signature != expected {
                return Err(AsterError::share_not_found(
                    "preview link token signature mismatch",
                ));
            }
            Ok(ResolvedPreviewTarget::File { payload, file })
        }
        PreviewSubject::ShareFile { share_token } => {
            let (share, file) = share_service::load_preview_shared_file(state, share_token).await?;
            let expected = sign_shared_payload(
                &share,
                &file,
                payload_segment,
                &state.config.auth.jwt_secret,
            );
            if signature != expected {
                return Err(AsterError::share_not_found(
                    "preview link token signature mismatch",
                ));
            }
            Ok(ResolvedPreviewTarget::Shared { payload, file })
        }
        PreviewSubject::ShareFolderFile {
            share_token,
            file_id,
        } => {
            let (share, file) =
                share_service::load_preview_shared_folder_file(state, share_token, *file_id)
                    .await?;
            let expected = sign_shared_payload(
                &share,
                &file,
                payload_segment,
                &state.config.auth.jwt_secret,
            );
            if signature != expected {
                return Err(AsterError::share_not_found(
                    "preview link token signature mismatch",
                ));
            }
            Ok(ResolvedPreviewTarget::Shared { payload, file })
        }
    }
}

fn split_token(token: &str) -> Result<(&str, &str)> {
    let (payload_segment, signature) = token
        .split_once('.')
        .ok_or_else(|| AsterError::share_not_found("invalid preview link token"))?;
    if payload_segment.is_empty() || signature.is_empty() {
        return Err(AsterError::share_not_found("invalid preview link token"));
    }
    Ok((payload_segment, signature))
}

fn decode_payload(payload_segment: &str) -> Result<PreviewTokenPayload> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_segment)
        .map_aster_err_with(|| AsterError::share_not_found("invalid preview link token"))?;
    serde_json::from_slice::<PreviewTokenPayload>(&bytes)
        .map_aster_err_with(|| AsterError::share_not_found("invalid preview link token"))
}

fn decode_expiry(exp: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp(exp, 0)
        .ok_or_else(|| AsterError::share_not_found("invalid preview link expiry"))
}

fn file_scope_signature(file: &file::Model) -> String {
    if let Some(team_id) = file.team_id {
        format!("team:{team_id}")
    } else {
        format!("user:{}", file.user_id)
    }
}

fn sign_file_payload(file: &file::Model, payload_segment: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        format!(
            "preview_link:file:{secret}:{}:{}:{payload_segment}",
            file_scope_signature(file),
            file.id
        )
        .as_bytes(),
    );
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn sign_shared_payload(
    share: &share::Model,
    file: &file::Model,
    payload_segment: &str,
    secret: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        format!(
            "preview_link:share:{secret}:{}:{}:{}:{payload_segment}",
            share.token, share.id, file.id
        )
        .as_bytes(),
    );
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

async fn reserve_usage(
    state: &AppState,
    token: &str,
    payload: &PreviewTokenPayload,
) -> Result<ReservedUse> {
    let cache_key = preview_cache_key(token);
    let ttl_secs = ttl_seconds(payload)?;
    let usage = state
        .cache
        .get::<PreviewUsageState>(&cache_key)
        .await
        .unwrap_or_default();
    if usage.used >= payload.max_uses {
        return Err(AsterError::share_download_limit(
            "preview link usage limit reached",
        ));
    }

    let next_used = usage.used.saturating_add(1);
    state
        .cache
        .set(
            &cache_key,
            &PreviewUsageState { used: next_used },
            Some(ttl_secs),
        )
        .await;

    Ok(ReservedUse {
        cache_key,
        previous_used: usage.used,
        ttl_secs,
    })
}

async fn rollback_usage(state: &AppState, reserved: &ReservedUse) {
    if reserved.previous_used == 0 {
        state.cache.delete(&reserved.cache_key).await;
        return;
    }

    state
        .cache
        .set(
            &reserved.cache_key,
            &PreviewUsageState {
                used: reserved.previous_used,
            },
            Some(reserved.ttl_secs),
        )
        .await;
}

fn ttl_seconds(payload: &PreviewTokenPayload) -> Result<u64> {
    let remaining = payload.exp.saturating_sub(Utc::now().timestamp());
    if remaining <= 0 {
        return Err(AsterError::share_expired("preview link expired"));
    }
    u64::try_from(remaining).map_aster_err_ctx(
        "preview link ttl conversion failed",
        AsterError::internal_error,
    )
}

fn preview_cache_key(token: &str) -> String {
    format!("{PREVIEW_LINK_CACHE_PREFIX}{token}")
}

#[cfg(test)]
mod tests {
    use super::{decode_payload, preview_path, split_token};
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
            category: "test".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn preview_path_encodes_file_name() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            preview_path(&runtime_config, "abc", "deck final.pptx"),
            "/pv/abc/deck%20final.pptx"
        );
    }

    #[test]
    fn preview_path_uses_public_site_url_when_configured() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            crate::config::site_url::PUBLIC_SITE_URL_KEY,
            "https://drive.example.com",
        ));

        assert_eq!(
            preview_path(&runtime_config, "abc", "deck final.pptx"),
            "https://drive.example.com/pv/abc/deck%20final.pptx"
        );
    }

    #[test]
    fn split_token_rejects_invalid_value() {
        assert!(split_token("invalid").is_err());
        assert!(split_token(".sig").is_err());
        assert!(split_token("payload.").is_err());
    }

    #[test]
    fn decode_payload_rejects_garbage() {
        assert!(decode_payload("%%%").is_err());
    }
}
