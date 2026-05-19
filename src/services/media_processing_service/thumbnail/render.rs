use std::sync::Arc;

use crate::config::operations;
use crate::entities::file_blob;
use crate::errors::Result;
use crate::runtime::PrimaryAppState;
use crate::storage::StorageDriver;
use crate::types::MediaProcessorKind;

use crate::services::media_processing_service::shared::ResolvedMediaProcessor;

use super::cli::{
    render_image_preview_with_vips_cli, render_thumbnail_with_ffmpeg_cli,
    render_thumbnail_with_vips_cli,
};
use super::storage_native::{
    render_image_preview_with_storage_native, render_thumbnail_with_storage_native,
};

pub(super) async fn render_thumbnail_bytes(
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
            crate::services::thumbnail_service::render_thumbnail_bytes(
                driver.as_ref(),
                blob,
                &state.config.server.temp_dir,
            )
            .await
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

pub(super) async fn render_image_preview_bytes(
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
                "rendering image preview via built-in images pipeline"
            );
            crate::services::thumbnail_service::ensure_source_size_supported(
                blob,
                operations::thumbnail_max_source_bytes(&state.runtime_config),
            )?;
            crate::services::thumbnail_service::render_webp_derivative_bytes(
                driver.as_ref(),
                blob,
                &state.config.server.temp_dir,
                crate::services::thumbnail_service::current_image_preview_max_dim(),
            )
            .await
        }
        MediaProcessorKind::VipsCli => {
            let command = processor.vips_command().to_string();
            tracing::debug!(
                blob_id = blob.id,
                processor = "vips_cli",
                command,
                "rendering image preview via vips CLI pipeline"
            );
            crate::services::thumbnail_service::ensure_source_size_supported(
                blob,
                operations::thumbnail_max_source_bytes(&state.runtime_config),
            )?;
            render_image_preview_with_vips_cli(
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
                "rendering image preview via storage-native pipeline"
            );
            render_image_preview_with_storage_native(blob, driver.as_ref(), source_mime_type).await
        }
        MediaProcessorKind::FfmpegCli => {
            let command = processor.ffmpeg_command().to_string();
            tracing::debug!(
                blob_id = blob.id,
                processor = "ffmpeg_cli",
                command,
                "rendering image preview via ffmpeg CLI pipeline"
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
    }
}
