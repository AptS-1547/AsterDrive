//! 媒体处理相关 runtime config。

mod registry;
#[cfg(test)]
mod tests;
mod types;

pub use crate::config::definitions::MEDIA_PROCESSING_REGISTRY_JSON_KEY;
pub use registry::default_processor_config_for_kind;
pub use registry::{
    builtin_images_supports_extension, command_is_available, default_media_processing_registry,
    default_media_processing_registry_json, file_extension, media_processing_registry,
    normalize_media_processing_registry_config_value, normalize_vips_command,
    parse_media_processor_kind, processor_candidates_for_file_name, processor_config_for_kind,
    public_thumbnail_support, vips_command_from_registry_value,
};
pub use types::{
    BUILTIN_IMAGES_SUPPORTED_EXTENSIONS, DEFAULT_VIPS_COMMAND, DEFAULT_VIPS_EXTENSIONS,
    MEDIA_PROCESSING_REGISTRY_VERSION, MatchedMediaProcessor, MediaProcessingMatchKind,
    MediaProcessingProcessorConfig, MediaProcessingProcessorRuntimeConfig,
    MediaProcessingRegistryConfig, PUBLIC_THUMBNAIL_SUPPORT_VERSION, PublicThumbnailSupport,
};
