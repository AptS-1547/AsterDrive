//! 统一媒体处理服务。
//!
//! 第一阶段只接入 thumbnail 场景，把业务层和具体处理实现解耦。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::{media_processing as media_processing_config, operations};
use crate::db::repository::file_repo;
use crate::entities::{file_blob, storage_policy};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::PrimaryAppState;
use crate::storage::{StorageDriver, extensions::NativeThumbnailRequest};
use crate::types::{MediaProcessorKind, parse_storage_policy_options};

const VIPS_CLI_THUMBNAIL_VERSION: &str = "vips-cli-v1";
const STORAGE_NATIVE_THUMBNAIL_VERSION: &str = "storage-native-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    Image,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedThumbnailProcessor {
    kind: MediaProcessorKind,
    vips_command: Option<String>,
}

impl ResolvedThumbnailProcessor {
    fn new(kind: MediaProcessorKind) -> Self {
        Self {
            kind,
            vips_command: None,
        }
    }

    fn with_vips_command(command: String) -> Self {
        Self {
            kind: MediaProcessorKind::VipsCli,
            vips_command: Some(command),
        }
    }

    pub(crate) fn kind(&self) -> MediaProcessorKind {
        self.kind
    }

    fn vips_command(&self) -> &str {
        self.vips_command
            .as_deref()
            .unwrap_or(media_processing_config::DEFAULT_VIPS_COMMAND)
    }

    fn thumbnail_version(&self) -> &'static str {
        match self.kind {
            MediaProcessorKind::Images => {
                crate::services::thumbnail_service::CURRENT_IMAGES_THUMBNAIL_VERSION
            }
            MediaProcessorKind::VipsCli => VIPS_CLI_THUMBNAIL_VERSION,
            MediaProcessorKind::StorageNative => STORAGE_NATIVE_THUMBNAIL_VERSION,
        }
    }

    fn cache_path(&self, blob_hash: &str) -> String {
        crate::services::thumbnail_service::thumb_path_for_version(
            blob_hash,
            self.thumbnail_version(),
        )
    }
}

pub struct ThumbnailData {
    pub data: Vec<u8>,
    pub thumbnail_version: String,
}

pub struct StoredThumbnail {
    pub thumbnail_path: String,
    pub thumbnail_version: String,
    pub reused_existing_thumbnail: bool,
}

struct ThumbnailContext {
    driver: Arc<dyn StorageDriver>,
    processor: ResolvedThumbnailProcessor,
}

pub async fn probe_vips_cli_command(command: &str) -> Result<String> {
    let command = media_processing_config::normalize_vips_command(command)?;
    if !media_processing_config::command_is_available(&command) {
        return Err(AsterError::validation_error(format!(
            "vips_cli command '{command}' is not available"
        )));
    }

    tracing::debug!(
        processor = "vips_cli",
        command = %command,
        "starting vips CLI probe"
    );

    let probe_command = command.clone();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&probe_command)
            .arg("--version")
            .output()
    })
    .await
    .map_aster_err_ctx(
        "vips CLI probe task panicked",
        AsterError::thumbnail_generation_failed,
    )?
    .map_err(|error| {
        AsterError::validation_error(format!("spawn vips_cli '{command}': {error}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit status {}", output.status)
        };
        return Err(AsterError::validation_error(format!(
            "vips_cli probe failed for '{command}': {detail}"
        )));
    }

    let detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !detail.is_empty() {
        detail
    } else {
        String::from_utf8_lossy(&output.stderr).trim().to_string()
    };

    tracing::debug!(
        processor = "vips_cli",
        command = %command,
        version = detail.as_str(),
        "vips CLI probe completed"
    );

    if detail.is_empty() {
        Ok(format!("vips_cli command '{command}' is available"))
    } else {
        Ok(format!(
            "vips_cli command '{command}' is available: {detail}"
        ))
    }
}

pub fn thumbnail_etag_value_for(blob_hash: &str, thumbnail_version: Option<&str>) -> String {
    crate::services::thumbnail_service::thumbnail_etag_value_for(blob_hash, thumbnail_version)
}

pub(crate) fn known_thumbnail_cache_paths(blob_hash: &str) -> Vec<String> {
    vec![
        crate::services::thumbnail_service::thumb_path(blob_hash),
        crate::services::thumbnail_service::thumb_path_for_version(
            blob_hash,
            crate::services::thumbnail_service::LEGACY_IMAGES_THUMBNAIL_VERSION,
        ),
        crate::services::thumbnail_service::thumb_path_for_version(
            blob_hash,
            VIPS_CLI_THUMBNAIL_VERSION,
        ),
        crate::services::thumbnail_service::thumb_path_for_version(
            blob_hash,
            STORAGE_NATIVE_THUMBNAIL_VERSION,
        ),
        crate::services::thumbnail_service::legacy_thumb_path(blob_hash),
    ]
}

pub(crate) fn resolve_thumbnail_processor_for_blob(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<ResolvedThumbnailProcessor> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    resolve_thumbnail_processor_for_policy(state, &policy, file_name, source_mime_type)
}

pub async fn load_thumbnail_if_exists(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<Option<ThumbnailData>> {
    let ctx = build_thumbnail_context(state, blob, file_name, source_mime_type)?;
    load_thumbnail_if_exists_with_context(state, blob, &ctx).await
}

pub async fn get_or_generate_thumbnail(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<ThumbnailData> {
    let ctx = build_thumbnail_context(state, blob, file_name, source_mime_type)?;
    if let Some(data) = load_thumbnail_if_exists_with_context(state, blob, &ctx).await? {
        return Ok(data);
    }

    let thumbnail_version = ctx.processor.thumbnail_version().to_string();
    let thumbnail_path = ctx.processor.cache_path(&blob.hash);
    let webp_bytes =
        render_thumbnail_bytes(state, blob, source_mime_type, &ctx.driver, &ctx.processor).await?;

    if let Err(error) = ctx.driver.put(&thumbnail_path, &webp_bytes).await {
        tracing::warn!("failed to store thumbnail {thumbnail_path}: {error}");
    } else if let Err(error) =
        file_repo::set_thumbnail_metadata(&state.db, blob.id, &thumbnail_path, &thumbnail_version)
            .await
    {
        tracing::warn!(
            blob_id = blob.id,
            path = thumbnail_path,
            "failed to persist thumbnail metadata after synchronous generation: {error}"
        );
    }

    Ok(ThumbnailData {
        data: webp_bytes,
        thumbnail_version,
    })
}

pub async fn generate_and_store_thumbnail(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<StoredThumbnail> {
    let ctx = build_thumbnail_context(state, blob, file_name, source_mime_type)?;
    generate_and_store_with_context(state, blob, source_mime_type, &ctx).await
}

pub(crate) async fn generate_and_store_thumbnail_with_processor(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    source_file_name: &str,
    source_mime_type: &str,
    processor_kind: MediaProcessorKind,
) -> Result<StoredThumbnail> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let ctx =
        build_thumbnail_context_with_processor(state, &policy, source_file_name, processor_kind)?;
    generate_and_store_with_context(state, blob, source_mime_type, &ctx).await
}

pub async fn delete_thumbnail(state: &PrimaryAppState, blob: &file_blob::Model) -> Result<()> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let mut paths = BTreeSet::new();
    if let Some(path) = blob.thumbnail_path.as_ref() {
        paths.insert(path.clone());
    }
    for path in known_thumbnail_cache_paths(&blob.hash) {
        paths.insert(path);
    }

    for path in paths {
        if driver.exists(&path).await.unwrap_or(false) {
            driver.delete(&path).await?;
        }
    }

    if let Err(error) = file_repo::clear_thumbnail_metadata(&state.db, blob.id).await {
        tracing::warn!(
            blob_id = blob.id,
            "failed to clear thumbnail metadata: {error}"
        );
    }
    Ok(())
}

fn media_kind_for_mime(mime: &str) -> MediaKind {
    if mime.trim().to_ascii_lowercase().starts_with("image/") {
        MediaKind::Image
    } else {
        MediaKind::Other
    }
}

fn resolve_thumbnail_processor_for_policy(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<ResolvedThumbnailProcessor> {
    let media_kind = media_kind_for_mime(source_mime_type);
    let registry = media_processing_config::media_processing_registry(&state.runtime_config);
    let source_extension = media_processing_config::file_extension(file_name);
    let mut last_unavailable_reason = None;
    let policy_options = parse_storage_policy_options(policy.options.as_ref());
    if policy_options.thumbnail_processor == Some(MediaProcessorKind::StorageNative) {
        let processor_config = media_processing_config::default_processor_config_for_kind(
            MediaProcessorKind::StorageNative,
        );
        let unavailable_reason = thumbnail_processor_unavailable_reason(
            state,
            policy,
            &processor_config,
            Some(file_name),
        )?;
        if let Some(reason) = unavailable_reason {
            tracing::debug!(
                operation = "thumbnail",
                policy_id = policy.id,
                file_name,
                source_extension = source_extension.as_deref().unwrap_or(""),
                processor = MediaProcessorKind::StorageNative.as_str(),
                processor_match = "policy",
                skip_reason = %reason,
                media_kind = ?media_kind,
                "skipped unavailable policy-native media processor"
            );
            last_unavailable_reason = Some(reason);
        } else {
            tracing::debug!(
                operation = "thumbnail",
                policy_id = policy.id,
                file_name,
                source_extension = source_extension.as_deref().unwrap_or(""),
                processor = MediaProcessorKind::StorageNative.as_str(),
                processor_match = "policy",
                media_kind = ?media_kind,
                "resolved media processor from storage policy"
            );
            return Ok(resolved_thumbnail_processor_from_config(&processor_config));
        }
    }

    let candidates = media_processing_config::thumbnail_processor_candidates(&registry, file_name);
    if candidates.is_empty() {
        let reason = last_unavailable_reason
            .unwrap_or_else(|| format!("no enabled thumbnail processor matched '{file_name}'"));
        return Err(AsterError::precondition_failed(reason));
    }

    for candidate in candidates {
        let unavailable_reason = thumbnail_processor_unavailable_reason(
            state,
            policy,
            &candidate.processor,
            Some(file_name),
        )?;
        if let Some(reason) = unavailable_reason {
            tracing::debug!(
                operation = "thumbnail",
                policy_id = policy.id,
                file_name,
                source_extension = source_extension.as_deref().unwrap_or(""),
                processor = candidate.processor.kind.as_str(),
                processor_match = candidate.match_kind.as_str(),
                skip_reason = %reason,
                media_kind = ?media_kind,
                "skipped unavailable media processor"
            );
            last_unavailable_reason = Some(reason);
            continue;
        }

        tracing::debug!(
            operation = "thumbnail",
            policy_id = policy.id,
            file_name,
            source_extension = source_extension.as_deref().unwrap_or(""),
            processor = candidate.processor.kind.as_str(),
            processor_match = candidate.match_kind.as_str(),
            media_kind = ?media_kind,
            "resolved media processor"
        );
        return Ok(resolved_thumbnail_processor_from_config(
            &candidate.processor,
        ));
    }

    let reason = last_unavailable_reason
        .unwrap_or_else(|| format!("no available thumbnail processor matched '{file_name}'"));
    Err(AsterError::precondition_failed(reason))
}

fn build_thumbnail_context(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<ThumbnailContext> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let processor =
        resolve_thumbnail_processor_for_policy(state, &policy, file_name, source_mime_type)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    Ok(ThumbnailContext { driver, processor })
}

fn build_thumbnail_context_with_processor(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    source_file_name: &str,
    processor_kind: MediaProcessorKind,
) -> Result<ThumbnailContext> {
    let registry = media_processing_config::media_processing_registry(&state.runtime_config);
    let processor_config =
        media_processing_config::processor_config_for_kind(&registry, processor_kind)
            .cloned()
            .unwrap_or_else(|| {
                media_processing_config::default_processor_config_for_kind(processor_kind)
            });
    let processor = resolved_thumbnail_processor_from_config(&processor_config);
    if let Some(reason) = thumbnail_processor_unavailable_reason(
        state,
        policy,
        &processor_config,
        Some(source_file_name),
    )? {
        return Err(AsterError::precondition_failed(reason));
    }

    let driver = state.driver_registry.get_driver(policy)?;
    let source_extension = media_processing_config::file_extension(source_file_name);
    tracing::debug!(
        operation = "thumbnail",
        policy_id = policy.id,
        processor = processor_kind.as_str(),
        selection_source = "task_payload",
        source_file_name,
        source_extension = source_extension.as_deref().unwrap_or(""),
        "built thumbnail context with explicit processor"
    );
    Ok(ThumbnailContext { driver, processor })
}

fn resolved_thumbnail_processor_from_config(
    processor: &media_processing_config::MediaProcessingProcessorConfig,
) -> ResolvedThumbnailProcessor {
    match processor.kind {
        MediaProcessorKind::VipsCli => ResolvedThumbnailProcessor::with_vips_command(
            processor
                .config
                .command
                .clone()
                .unwrap_or_else(|| media_processing_config::DEFAULT_VIPS_COMMAND.to_string()),
        ),
        _ => ResolvedThumbnailProcessor::new(processor.kind),
    }
}

fn thumbnail_processor_unavailable_reason(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    processor: &media_processing_config::MediaProcessingProcessorConfig,
    source_file_name: Option<&str>,
) -> Result<Option<String>> {
    match processor.kind {
        MediaProcessorKind::Images => {
            match source_file_name.and_then(media_processing_config::file_extension) {
                Some(extension)
                    if !media_processing_config::builtin_images_supports_extension(&extension) =>
                {
                    return Ok(Some(format!(
                        "built-in images processor does not support file extension '{extension}'"
                    )));
                }
                None => {
                    return Ok(Some(
                        "built-in images processor requires a supported file extension".to_string(),
                    ));
                }
                Some(_) => {}
            }
            Ok(None)
        }
        MediaProcessorKind::VipsCli => {
            let command = processor
                .config
                .command
                .as_deref()
                .unwrap_or(media_processing_config::DEFAULT_VIPS_COMMAND);
            if !media_processing_config::command_is_available(command) {
                return Ok(Some(format!(
                    "thumbnail vips CLI command '{command}' is not available"
                )));
            }
            Ok(None)
        }
        MediaProcessorKind::StorageNative => {
            let driver = state.driver_registry.get_driver(policy)?;
            if driver.as_native_thumbnail().is_none() {
                return Ok(Some(format!(
                    "storage policy #{} does not expose storage-native thumbnail processing",
                    policy.id
                )));
            }
            Ok(None)
        }
    }
}

async fn load_thumbnail_if_exists_with_context(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    ctx: &ThumbnailContext,
) -> Result<Option<ThumbnailData>> {
    if requires_server_side_source_limit(&ctx.processor) {
        crate::services::thumbnail_service::ensure_source_size_supported(
            blob,
            operations::thumbnail_max_source_bytes(&state.runtime_config),
        )?;
    }

    let expected_version = ctx.processor.thumbnail_version();
    if blob.thumbnail_version.as_deref() != Some(expected_version)
        && (blob.thumbnail_path.is_some() || blob.thumbnail_version.is_some())
    {
        tracing::debug!(
            blob_id = blob.id,
            processor = ctx.processor.kind().as_str(),
            persisted_thumbnail_version = blob.thumbnail_version.as_deref(),
            expected_thumbnail_version = expected_version,
            "clearing stale thumbnail metadata before loading"
        );
        clear_thumbnail_metadata(state, blob).await;
    }

    if blob.thumbnail_version.as_deref() == Some(expected_version)
        && let Some(path) = blob.thumbnail_path.as_deref()
        && let Some(data) = load_thumbnail_from_path(state, blob, &ctx.driver, path, true).await?
    {
        tracing::debug!(
            blob_id = blob.id,
            processor = ctx.processor.kind().as_str(),
            thumbnail_path = path,
            thumbnail_version = expected_version,
            cache_source = "persisted_metadata",
            "thumbnail cache hit"
        );
        return Ok(Some(ThumbnailData {
            data,
            thumbnail_version: expected_version.to_string(),
        }));
    }

    let expected_path = ctx.processor.cache_path(&blob.hash);
    if let Some(data) =
        load_thumbnail_from_path(state, blob, &ctx.driver, &expected_path, false).await?
    {
        tracing::debug!(
            blob_id = blob.id,
            processor = ctx.processor.kind().as_str(),
            thumbnail_path = expected_path,
            thumbnail_version = expected_version,
            cache_source = "computed_path",
            "thumbnail cache hit"
        );
        persist_thumbnail_metadata(state, blob, &expected_path, expected_version).await;
        return Ok(Some(ThumbnailData {
            data,
            thumbnail_version: expected_version.to_string(),
        }));
    }

    Ok(None)
}

async fn generate_and_store_with_context(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    source_mime_type: &str,
    ctx: &ThumbnailContext,
) -> Result<StoredThumbnail> {
    if let Some(existing) = load_thumbnail_if_exists_with_context(state, blob, ctx).await? {
        tracing::debug!(
            blob_id = blob.id,
            processor = ctx.processor.kind().as_str(),
            thumbnail_version = existing.thumbnail_version,
            "reusing existing thumbnail without rendering"
        );
        return Ok(StoredThumbnail {
            thumbnail_path: ctx.processor.cache_path(&blob.hash),
            thumbnail_version: existing.thumbnail_version,
            reused_existing_thumbnail: true,
        });
    }

    let thumbnail_version = ctx.processor.thumbnail_version().to_string();
    let thumbnail_path = ctx.processor.cache_path(&blob.hash);
    tracing::debug!(
        blob_id = blob.id,
        processor = ctx.processor.kind().as_str(),
        thumbnail_path,
        thumbnail_version,
        "rendering thumbnail because cache miss"
    );
    let webp_bytes =
        render_thumbnail_bytes(state, blob, source_mime_type, &ctx.driver, &ctx.processor).await?;
    let stored_path = ctx.driver.put(&thumbnail_path, &webp_bytes).await?;
    file_repo::set_thumbnail_metadata(&state.db, blob.id, &stored_path, &thumbnail_version).await?;

    tracing::debug!(
        blob_id = blob.id,
        processor = ctx.processor.kind().as_str(),
        stored_path,
        thumbnail_version,
        bytes = webp_bytes.len(),
        "thumbnail rendered and stored"
    );

    Ok(StoredThumbnail {
        thumbnail_path: stored_path,
        thumbnail_version,
        reused_existing_thumbnail: false,
    })
}

async fn render_thumbnail_bytes(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    source_mime_type: &str,
    driver: &Arc<dyn StorageDriver>,
    processor: &ResolvedThumbnailProcessor,
) -> Result<Vec<u8>> {
    match processor.kind() {
        MediaProcessorKind::Images => {
            tracing::debug!(
                blob_id = blob.id,
                processor = "images",
                "rendering thumbnail via built-in images pipeline"
            );
            crate::services::thumbnail_service::ensure_source_size_supported(
                blob,
                operations::thumbnail_max_source_bytes(&state.runtime_config),
            )?;
            crate::services::thumbnail_service::render_thumbnail_bytes(driver.as_ref(), blob).await
        }
        MediaProcessorKind::VipsCli => {
            let command = processor.vips_command().to_string();
            tracing::debug!(
                blob_id = blob.id,
                processor = "vips_cli",
                command,
                "rendering thumbnail via vips CLI pipeline"
            );
            crate::services::thumbnail_service::ensure_source_size_supported(
                blob,
                operations::thumbnail_max_source_bytes(&state.runtime_config),
            )?;
            render_thumbnail_with_vips_cli(state, blob, driver.as_ref(), &command).await
        }
        MediaProcessorKind::StorageNative => {
            tracing::debug!(
                blob_id = blob.id,
                processor = "storage_native",
                "rendering thumbnail via storage-native pipeline"
            );
            render_thumbnail_with_storage_native(blob, driver.as_ref(), source_mime_type).await
        }
    }
}

async fn render_thumbnail_with_storage_native(
    blob: &file_blob::Model,
    driver: &dyn StorageDriver,
    source_mime_type: &str,
) -> Result<Vec<u8>> {
    let native = driver.as_native_thumbnail().ok_or_else(|| {
        AsterError::precondition_failed(
            "storage driver does not support native thumbnail processing",
        )
    })?;
    let bytes = native
        .get_native_thumbnail(&NativeThumbnailRequest {
            storage_path: blob.storage_path.clone(),
            source_mime_type: source_mime_type.to_string(),
            max_width: crate::services::thumbnail_service::current_thumbnail_max_dim(),
            max_height: crate::services::thumbnail_service::current_thumbnail_max_dim(),
        })
        .await?
        .ok_or_else(|| {
            AsterError::precondition_failed("storage driver could not produce a native thumbnail")
        })?;
    tracing::debug!(
        blob_id = blob.id,
        processor = "storage_native",
        bytes = bytes.len(),
        "storage-native thumbnail render completed"
    );
    Ok(bytes)
}

async fn render_thumbnail_with_vips_cli(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    driver: &dyn StorageDriver,
    command: &str,
) -> Result<Vec<u8>> {
    let original = driver.get(&blob.storage_path).await?;
    let temp_root = crate::utils::paths::runtime_temp_dir(&state.config.server.temp_dir);
    let temp_dir = PathBuf::from(temp_root).join(format!("media-vips-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create vips temp dir", AsterError::storage_driver_error)?;

    let input_path = temp_dir.join("source");
    let output_path = temp_dir.join("thumbnail.webp");
    tokio::fs::write(&input_path, original)
        .await
        .map_aster_err_ctx(
            "write vips source temp file",
            AsterError::thumbnail_generation_failed,
        )?;

    let command = command.to_string();
    let input_arg = input_path.to_string_lossy().to_string();
    let output_arg = output_path.to_string_lossy().to_string();
    let max_dim = crate::services::thumbnail_service::current_thumbnail_max_dim();
    tracing::debug!(
        blob_id = blob.id,
        processor = "vips_cli",
        command,
        input_path = input_arg,
        output_path = output_arg,
        max_dim,
        "starting vips CLI thumbnail render"
    );
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(&command)
            .arg("thumbnail")
            .arg(&input_arg)
            .arg(&output_arg)
            .arg(max_dim.to_string())
            .arg("--height")
            .arg(max_dim.to_string())
            .output()
            .map_err(|error| {
                AsterError::thumbnail_generation_failed(format!(
                    "spawn vips CLI '{command}': {error}"
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit status {}", output.status)
            };
            return Err(AsterError::thumbnail_generation_failed(format!(
                "vips CLI thumbnail command failed: {detail}"
            )));
        }
        Ok::<(), AsterError>(())
    })
    .await
    .map_aster_err_ctx(
        "vips CLI thumbnail task panicked",
        AsterError::thumbnail_generation_failed,
    )??;

    let thumbnail = tokio::fs::read(&output_path).await.map_aster_err_ctx(
        "read vips thumbnail output",
        AsterError::thumbnail_generation_failed,
    );
    crate::utils::cleanup_temp_dir(temp_dir.to_string_lossy().as_ref()).await;
    if let Ok(bytes) = &thumbnail {
        tracing::debug!(
            blob_id = blob.id,
            processor = "vips_cli",
            bytes = bytes.len(),
            "vips CLI thumbnail render completed"
        );
    }
    thumbnail
}

async fn load_thumbnail_from_path(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    driver: &Arc<dyn StorageDriver>,
    path: &str,
    clear_metadata_on_missing: bool,
) -> Result<Option<Vec<u8>>> {
    match driver.get(path).await {
        Ok(data) => Ok(Some(data)),
        Err(error) => match driver.exists(path).await {
            Ok(false) => {
                if clear_metadata_on_missing {
                    clear_thumbnail_metadata(state, blob).await;
                }
                Ok(None)
            }
            Ok(true) => Err(error),
            Err(exists_error) => {
                tracing::warn!(
                    blob_id = blob.id,
                    path,
                    "thumbnail get failed and existence recheck also failed: {exists_error}"
                );
                Err(error)
            }
        },
    }
}

async fn clear_thumbnail_metadata(state: &PrimaryAppState, blob: &file_blob::Model) {
    if let Err(error) = file_repo::clear_thumbnail_metadata(&state.db, blob.id).await {
        tracing::warn!(
            blob_id = blob.id,
            "failed to clear stale thumbnail metadata: {error}"
        );
    }
}

async fn persist_thumbnail_metadata(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    path: &str,
    version: &str,
) {
    if let Err(error) = file_repo::set_thumbnail_metadata(&state.db, blob.id, path, version).await {
        tracing::warn!(
            blob_id = blob.id,
            path,
            "failed to persist thumbnail metadata: {error}"
        );
    }
}

fn requires_server_side_source_limit(processor: &ResolvedThumbnailProcessor) -> bool {
    processor.kind() != MediaProcessorKind::StorageNative
}

#[cfg(test)]
mod tests {
    use super::{MediaKind, known_thumbnail_cache_paths, media_kind_for_mime};
    use crate::config::media_processing::command_is_available;

    #[test]
    fn known_thumbnail_cache_paths_include_current_and_legacy_namespaces() {
        let hash = "abc".repeat(21) + "a";
        let paths = known_thumbnail_cache_paths(&hash);
        assert!(paths.contains(&format!("_thumb/images-v1/ab/ca/{hash}.webp")));
        assert!(paths.contains(&format!("_thumb/v2/ab/ca/{hash}.webp")));
        assert!(paths.contains(&format!("_thumb/vips-cli-v1/ab/ca/{hash}.webp")));
        assert!(paths.contains(&format!("_thumb/storage-native-v1/ab/ca/{hash}.webp")));
    }

    #[test]
    fn media_kind_classifies_thumbnail_mimes() {
        assert_eq!(media_kind_for_mime("image/png"), MediaKind::Image);
        assert_eq!(media_kind_for_mime("application/pdf"), MediaKind::Other);
    }

    #[test]
    fn command_is_available_rejects_blank_command() {
        assert!(!command_is_available(""));
        assert!(!command_is_available("   "));
    }
}
