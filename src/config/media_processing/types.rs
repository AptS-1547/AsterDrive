use crate::types::MediaProcessorKind;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

pub const MEDIA_PROCESSING_REGISTRY_VERSION: i32 = 1;
pub const PUBLIC_THUMBNAIL_SUPPORT_VERSION: i32 = 1;
pub const DEFAULT_VIPS_COMMAND: &str = "vips";
pub const BUILTIN_IMAGES_SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "webp", "bmp", "tif", "tiff",
];
/// Common libvips input suffixes used as the default binding for `vips_cli`.
///
/// Actual availability still depends on how libvips was built on the server.
pub const DEFAULT_VIPS_EXTENSIONS: &[&str] = &[
    "csv", "mat", "img", "hdr", "pbm", "pgm", "ppm", "pfm", "pnm", "svg", "svgz", "j2k", "jp2",
    "jpt", "j2c", "jpc", "gif", "png", "jpg", "jpeg", "jpe", "webp", "tif", "tiff", "fits", "fit",
    "fts", "exr", "jxl", "pdf", "heic", "heif", "avif", "svs", "vms", "vmu", "ndpi", "scn", "mrxs",
    "svslide", "bif", "raw",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PublicThumbnailSupport {
    pub version: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaProcessingProcessorRuntimeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl MediaProcessingProcessorRuntimeConfig {
    fn is_empty(&self) -> bool {
        self.command.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaProcessingRegistryConfig {
    #[serde(default = "default_media_processing_registry_version")]
    pub version: i32,
    #[serde(default)]
    pub processors: Vec<MediaProcessingProcessorConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaProcessingProcessorConfig {
    pub kind: MediaProcessorKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "MediaProcessingProcessorRuntimeConfig::is_empty"
    )]
    pub config: MediaProcessingProcessorRuntimeConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaProcessingMatchKind {
    Policy,
    Extension,
    Any,
}

impl MediaProcessingMatchKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Extension => "extension",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedMediaProcessor {
    pub processor: MediaProcessingProcessorConfig,
    pub match_kind: MediaProcessingMatchKind,
}

const fn default_media_processing_registry_version() -> i32 {
    MEDIA_PROCESSING_REGISTRY_VERSION
}

const fn default_true() -> bool {
    true
}
