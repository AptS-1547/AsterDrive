use std::sync::Arc;

use crate::config::media_processing as media_processing_config;
use crate::storage::StorageDriver;
use crate::types::MediaProcessorKind;

pub(crate) const VIPS_CLI_THUMBNAIL_VERSION: &str = "vips-cli-v1";
pub(crate) const STORAGE_NATIVE_THUMBNAIL_VERSION: &str = "storage-native-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaOperation {
    Thumbnail,
    Avatar,
}

impl MediaOperation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Avatar => "avatar",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedMediaProcessor {
    kind: MediaProcessorKind,
    vips_command: Option<String>,
}

impl ResolvedMediaProcessor {
    pub(crate) fn new(kind: MediaProcessorKind) -> Self {
        Self {
            kind,
            vips_command: None,
        }
    }

    pub(crate) fn with_vips_command(command: String) -> Self {
        Self {
            kind: MediaProcessorKind::VipsCli,
            vips_command: Some(command),
        }
    }

    pub(crate) fn kind(&self) -> MediaProcessorKind {
        self.kind
    }

    pub(crate) fn vips_command(&self) -> &str {
        self.vips_command
            .as_deref()
            .unwrap_or(media_processing_config::DEFAULT_VIPS_COMMAND)
    }

    pub(crate) fn thumbnail_version(&self) -> &'static str {
        match self.kind {
            MediaProcessorKind::Images => {
                crate::services::thumbnail_service::CURRENT_IMAGES_THUMBNAIL_VERSION
            }
            MediaProcessorKind::VipsCli => VIPS_CLI_THUMBNAIL_VERSION,
            MediaProcessorKind::StorageNative => STORAGE_NATIVE_THUMBNAIL_VERSION,
        }
    }

    pub(crate) fn cache_path(&self, blob_hash: &str) -> String {
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

#[derive(Debug)]
pub struct ProcessedAvatar {
    pub small_bytes: Vec<u8>,
    pub large_bytes: Vec<u8>,
}

pub(crate) struct ThumbnailContext {
    pub(crate) driver: Arc<dyn StorageDriver>,
    pub(crate) processor: ResolvedMediaProcessor,
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

pub(crate) fn requires_server_side_source_limit(processor: &ResolvedMediaProcessor) -> bool {
    processor.kind() != MediaProcessorKind::StorageNative
}
