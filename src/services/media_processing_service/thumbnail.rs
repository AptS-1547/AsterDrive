use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::media_processing as media_processing_config;
use crate::config::operations;
use crate::db::repository::file_repo;
use crate::entities::file_blob;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::PrimaryAppState;
use crate::storage::{StorageDriver, extensions::NativeThumbnailRequest};
use crate::types::MediaProcessorKind;
use image::ImageFormat;

use super::cli_input::prepare_cli_source;
use super::resolve::{build_thumbnail_context, build_thumbnail_context_with_processor};
use super::shared::{
    ResolvedMediaProcessor, StoredThumbnail, ThumbnailContext, ThumbnailData,
    requires_server_side_source_limit,
};

const FFMPEG_THUMBNAIL_BATCH_SIZE: u32 = 50;

pub async fn probe_ffmpeg_cli_command(command: &str) -> Result<String> {
    let command = media_processing_config::normalize_ffmpeg_command(command)?;
    if !media_processing_config::command_is_available(&command) {
        return Err(AsterError::validation_error(format!(
            "ffmpeg_cli command '{command}' is not available"
        )));
    }

    tracing::debug!(
        processor = "ffmpeg_cli",
        command = %command,
        "starting ffmpeg CLI probe"
    );

    let probe_command = command.clone();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&probe_command)
            .arg("-version")
            .output()
    })
    .await
    .map_aster_err_ctx(
        "ffmpeg CLI probe task panicked",
        AsterError::thumbnail_generation_failed,
    )?
    .map_err(|error| {
        AsterError::validation_error(format!("spawn ffmpeg_cli '{command}': {error}"))
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
            "ffmpeg_cli probe failed for '{command}': {detail}"
        )));
    }

    let detail = first_non_empty_output_line(&output.stdout)
        .or_else(|| first_non_empty_output_line(&output.stderr))
        .unwrap_or_default();

    tracing::debug!(
        processor = "ffmpeg_cli",
        command = %command,
        version = detail.as_str(),
        "ffmpeg CLI probe completed"
    );

    if detail.is_empty() {
        Ok(format!("ffmpeg_cli command '{command}' is available"))
    } else {
        Ok(format!(
            "ffmpeg_cli command '{command}' is available: {detail}"
        ))
    }
}

pub async fn load_thumbnail_if_exists(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
) -> Result<Option<ThumbnailData>> {
    let ctx = build_thumbnail_context(state, blob, file_name)?;
    load_thumbnail_if_exists_with_context(state, blob, &ctx).await
}

pub async fn get_or_generate_thumbnail(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
    source_mime_type: &str,
) -> Result<ThumbnailData> {
    let ctx = build_thumbnail_context(state, blob, file_name)?;
    if let Some(data) = load_thumbnail_if_exists_with_context(state, blob, &ctx).await? {
        return Ok(data);
    }

    let thumbnail_version = ctx.processor.thumbnail_version().to_string();
    let thumbnail_path = ctx.processor.cache_path(&blob.hash);
    let webp_bytes = render_thumbnail_bytes(
        state,
        blob,
        file_name,
        source_mime_type,
        &ctx.driver,
        &ctx.processor,
    )
    .await?;

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
    let ctx = build_thumbnail_context(state, blob, file_name)?;
    generate_and_store_with_context(state, blob, file_name, source_mime_type, &ctx).await
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
    generate_and_store_with_context(state, blob, source_file_name, source_mime_type, &ctx).await
}

pub async fn delete_thumbnail(state: &PrimaryAppState, blob: &file_blob::Model) -> Result<()> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;

    let mut paths = BTreeSet::new();
    if let Some(path) = blob.thumbnail_path.as_ref() {
        paths.insert(path.clone());
    }
    for path in super::shared::known_thumbnail_cache_paths(&blob.hash) {
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
    source_file_name: &str,
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
    let webp_bytes = render_thumbnail_bytes(
        state,
        blob,
        source_file_name,
        source_mime_type,
        &ctx.driver,
        &ctx.processor,
    )
    .await?;
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
    source_file_name: &str,
    source_mime_type: &str,
    driver: &Arc<dyn StorageDriver>,
    processor: &ResolvedMediaProcessor,
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
            render_thumbnail_with_vips_cli(
                state,
                blob,
                source_file_name,
                source_mime_type,
                driver.as_ref(),
                &command,
            )
            .await
        }
        MediaProcessorKind::FfmpegCli => {
            let command = processor.ffmpeg_command().to_string();
            tracing::debug!(
                blob_id = blob.id,
                processor = "ffmpeg_cli",
                command,
                "rendering thumbnail via ffmpeg CLI pipeline"
            );
            crate::services::thumbnail_service::ensure_source_size_supported(
                blob,
                operations::thumbnail_max_source_bytes(&state.runtime_config),
            )?;
            render_thumbnail_with_ffmpeg_cli(
                state,
                blob,
                source_file_name,
                source_mime_type,
                driver.as_ref(),
                &command,
            )
            .await
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
    source_file_name: &str,
    source_mime_type: &str,
    driver: &dyn StorageDriver,
    command: &str,
) -> Result<Vec<u8>> {
    let temp_root = crate::utils::paths::runtime_temp_dir(&state.config.server.temp_dir);
    let temp_dir = PathBuf::from(temp_root).join(format!("media-vips-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create vips temp dir", AsterError::storage_driver_error)?;

    let output_path = temp_dir.join("thumbnail.webp");
    let prepared_input = prepare_cli_source(
        driver,
        &blob.storage_path,
        source_file_name,
        source_mime_type,
        &temp_dir,
        false,
    )
    .await?;

    let command = command.to_string();
    let input_arg = prepared_input.input_arg().to_string();
    let output_arg = output_path.to_string_lossy().to_string();
    let max_dim = crate::services::thumbnail_service::current_thumbnail_max_dim();
    tracing::debug!(
        blob_id = blob.id,
        processor = "vips_cli",
        command,
        input_arg = input_arg,
        input_source = prepared_input.kind().as_str(),
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
            .arg("--size")
            .arg("down")
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

async fn render_thumbnail_with_ffmpeg_cli(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    source_file_name: &str,
    source_mime_type: &str,
    driver: &dyn StorageDriver,
    command: &str,
) -> Result<Vec<u8>> {
    let temp_root = crate::utils::paths::runtime_temp_dir(&state.config.server.temp_dir);
    let temp_dir = PathBuf::from(temp_root).join(format!("media-ffmpeg-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_aster_err_ctx("create ffmpeg temp dir", AsterError::storage_driver_error)?;

    let output_path = temp_dir.join("thumbnail.png");
    let prepared_input = prepare_cli_source(
        driver,
        &blob.storage_path,
        source_file_name,
        source_mime_type,
        &temp_dir,
        true,
    )
    .await?;

    let command = command.to_string();
    let input_arg = prepared_input.input_arg().to_string();
    let output_arg = output_path.to_string_lossy().to_string();
    let max_dim = crate::services::thumbnail_service::current_thumbnail_max_dim();
    let filter_arg = format!(
        "thumbnail={FFMPEG_THUMBNAIL_BATCH_SIZE}:log=quiet,scale=min(iw\\,{max_dim}):min(ih\\,{max_dim}):force_original_aspect_ratio=decrease"
    );
    tracing::debug!(
        blob_id = blob.id,
        processor = "ffmpeg_cli",
        command,
        input_arg = input_arg,
        input_source = prepared_input.kind().as_str(),
        output_path = output_arg,
        max_dim,
        "starting ffmpeg CLI thumbnail render"
    );
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(&command)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostdin")
            .arg("-i")
            .arg(&input_arg)
            .arg("-map")
            .arg("0:v:0")
            .arg("-vf")
            .arg(&filter_arg)
            .arg("-frames:v")
            .arg("1")
            .arg("-an")
            .arg("-sn")
            .arg("-c:v")
            .arg("png")
            .arg("-y")
            .arg(&output_arg)
            .output()
            .map_err(|error| {
                AsterError::thumbnail_generation_failed(format!(
                    "spawn ffmpeg CLI '{command}': {error}"
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
                "ffmpeg CLI thumbnail command failed: {detail}"
            )));
        }
        Ok::<(), AsterError>(())
    })
    .await
    .map_aster_err_ctx(
        "ffmpeg CLI thumbnail task panicked",
        AsterError::thumbnail_generation_failed,
    )??;

    let thumbnail_png = tokio::fs::read(&output_path).await.map_aster_err_ctx(
        "read ffmpeg thumbnail output",
        AsterError::thumbnail_generation_failed,
    );
    let thumbnail = match thumbnail_png {
        Ok(bytes) => tokio::task::spawn_blocking(move || encode_webp_from_image_bytes(bytes))
            .await
            .map_aster_err_ctx(
                "ffmpeg thumbnail webp encode task panicked",
                AsterError::thumbnail_generation_failed,
            )?,
        Err(error) => Err(error),
    };
    crate::utils::cleanup_temp_dir(temp_dir.to_string_lossy().as_ref()).await;
    if let Ok(bytes) = &thumbnail {
        tracing::debug!(
            blob_id = blob.id,
            processor = "ffmpeg_cli",
            bytes = bytes.len(),
            "ffmpeg CLI thumbnail render completed"
        );
    }
    thumbnail
}

fn first_non_empty_output_line(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn encode_webp_from_image_bytes(bytes: Vec<u8>) -> Result<Vec<u8>> {
    let image = image::load_from_memory(&bytes).map_aster_err_ctx(
        "decode ffmpeg thumbnail output",
        AsterError::thumbnail_generation_failed,
    )?;
    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, ImageFormat::WebP)
        .map_aster_err_ctx(
            "encode ffmpeg thumbnail webp",
            AsterError::thumbnail_generation_failed,
        )?;
    Ok(buf.into_inner())
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
