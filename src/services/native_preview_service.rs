//! 存储侧外部预览服务。

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::time::Duration;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::db::repository::file_repo;
use crate::entities::{file, file_blob, storage_policy};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::services::{
    preview_app_service::{self, PreviewAppProvider, PreviewOpenMode},
    share_service,
    workspace_storage_service::WorkspaceStorageScope,
};
use crate::storage::traits::extensions::{
    NativePreviewMode, NativePreviewOpenMode, NativePreviewRequest, NativePreviewResult,
};
use crate::types::parse_storage_policy_options;

const NATIVE_PREVIEW_TTL_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct NativePreviewSession {
    pub action_url: String,
    pub provider: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub expires_at: DateTime<Utc>,
    pub mode: NativePreviewOpenMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl From<NativePreviewResult> for NativePreviewSession {
    fn from(value: NativePreviewResult) -> Self {
        Self {
            action_url: value.url,
            provider: value.provider,
            expires_at: value.expires_at,
            mode: value.open_mode,
            cache_key: value.cache_key,
            version: value.version,
        }
    }
}

pub(crate) async fn create_for_file_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    app_key: &str,
) -> Result<NativePreviewSession> {
    let file = crate::services::workspace_storage_service::verify_file_access_for_read(
        state, scope, file_id,
    )
    .await?;
    create_for_file(state, &file, app_key).await
}

pub async fn create_for_shared_file(
    state: &PrimaryAppState,
    share_token: &str,
    app_key: &str,
) -> Result<NativePreviewSession> {
    let (_share, file) = share_service::load_preview_shared_file(state, share_token).await?;
    create_for_file(state, &file, app_key).await
}

pub async fn create_for_shared_folder_file(
    state: &PrimaryAppState,
    share_token: &str,
    file_id: i64,
    app_key: &str,
) -> Result<NativePreviewSession> {
    let (_share, file) =
        share_service::load_preview_shared_folder_file(state, share_token, file_id).await?;
    create_for_file(state, &file, app_key).await
}

async fn create_for_file(
    state: &PrimaryAppState,
    file: &file::Model,
    app_key: &str,
) -> Result<NativePreviewSession> {
    let open_mode = resolve_native_preview_open_mode(state, app_key)?;
    let blob = file_repo::find_blob_by_id(state.reader_db(), file.blob_id).await?;
    create_for_file_and_blob(state, file, &blob, open_mode).await
}

async fn create_for_file_and_blob(
    state: &PrimaryAppState,
    file: &file::Model,
    blob: &file_blob::Model,
    open_mode: NativePreviewOpenMode,
) -> Result<NativePreviewSession> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    ensure_storage_native_processing_enabled(&policy)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let native_preview = driver.as_native_preview().ok_or_else(|| {
        AsterError::validation_error(format!(
            "storage policy '{}' does not support storage-native preview",
            policy.name
        ))
    })?;
    let request = NativePreviewRequest {
        storage_path: blob.storage_path.clone(),
        source_file_name: file.name.clone(),
        source_mime_type: file.mime_type.clone(),
        mode: NativePreviewMode::HtmlDocument,
        expires: Duration::from_secs(NATIVE_PREVIEW_TTL_SECS),
    };

    let mut session: NativePreviewSession = native_preview
        .create_native_preview(&request)
        .await?
        .map(Into::into)
        .ok_or_else(|| {
            AsterError::validation_error(format!(
                "file '{}' is not supported by storage-native preview",
                file.name
            ))
        })?;
    session.mode = open_mode;
    Ok(session)
}

fn ensure_storage_native_processing_enabled(policy: &storage_policy::Model) -> Result<()> {
    if parse_storage_policy_options(policy.options.as_ref()).storage_native_processing_enabled() {
        return Ok(());
    }

    Err(AsterError::validation_error(format!(
        "storage-native processing is disabled for storage policy '{}'",
        policy.name
    )))
}

fn resolve_native_preview_open_mode(
    state: &PrimaryAppState,
    app_key: &str,
) -> Result<NativePreviewOpenMode> {
    let app = preview_app_service::get_public_preview_apps(state)
        .apps
        .into_iter()
        .find(|candidate| candidate.key == app_key)
        .ok_or_else(|| AsterError::record_not_found(format!("preview app '{app_key}'")))?;

    if app.provider != PreviewAppProvider::NativePreview {
        return Err(AsterError::validation_error(format!(
            "preview app '{}' is not a storage-native preview provider",
            app.key
        )));
    }

    match app.config.mode.unwrap_or(PreviewOpenMode::Iframe) {
        PreviewOpenMode::Iframe => Ok(NativePreviewOpenMode::Iframe),
        PreviewOpenMode::NewTab => Ok(NativePreviewOpenMode::NewTab),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::types::{DriverType, StoredStoragePolicyAllowedTypes, StoredStoragePolicyOptions};

    fn policy_with_options(options: &str) -> storage_policy::Model {
        storage_policy::Model {
            id: 1,
            name: "Tencent COS".to_string(),
            driver_type: DriverType::TencentCos,
            endpoint: "https://cos.ap-guangzhou.myqcloud.com".to_string(),
            bucket: "bucket-1250000000".to_string(),
            access_key: "AKIDEXAMPLE".to_string(),
            secret_key: "SECRETEXAMPLE".to_string(),
            base_path: String::new(),
            remote_node_id: None,
            max_file_size: 0,
            allowed_types: StoredStoragePolicyAllowedTypes::empty(),
            options: StoredStoragePolicyOptions(options.to_string()),
            is_default: false,
            chunk_size: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn storage_native_preview_requires_policy_switch() {
        let policy = policy_with_options("{}");

        let error = ensure_storage_native_processing_enabled(&policy).unwrap_err();

        assert!(
            error
                .message()
                .contains("storage-native processing is disabled")
        );
    }

    #[test]
    fn storage_native_preview_allows_enabled_policy() {
        let policy = policy_with_options(r#"{"storage_native_processing_enabled":true}"#);

        ensure_storage_native_processing_enabled(&policy)
            .expect("enabled storage-native processing should pass");
    }
}
