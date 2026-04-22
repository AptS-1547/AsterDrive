//! 媒体处理相关 runtime config。

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::config::RuntimeConfig;
pub use crate::config::definitions::MEDIA_PROCESSING_REGISTRY_JSON_KEY;
use crate::errors::{AsterError, Result};
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

const THUMBNAIL_PROCESSOR_PRIORITY: [MediaProcessorKind; 2] =
    [MediaProcessorKind::VipsCli, MediaProcessorKind::Images];

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
    Extension,
    Any,
}

impl MediaProcessingMatchKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extension => "extension",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedThumbnailProcessor {
    pub processor: MediaProcessingProcessorConfig,
    pub match_kind: MediaProcessingMatchKind,
}

const fn default_media_processing_registry_version() -> i32 {
    MEDIA_PROCESSING_REGISTRY_VERSION
}

const fn default_true() -> bool {
    true
}

pub fn parse_media_processor_kind(value: &str) -> Option<MediaProcessorKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "images" => Some(MediaProcessorKind::Images),
        "vips_cli" => Some(MediaProcessorKind::VipsCli),
        "storage_native" => Some(MediaProcessorKind::StorageNative),
        _ => None,
    }
}

pub fn normalize_vips_command(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_VIPS_COMMAND.to_string());
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(AsterError::validation_error(
            "vips command cannot contain control characters",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn builtin_images_supports_extension(extension: &str) -> bool {
    BUILTIN_IMAGES_SUPPORTED_EXTENSIONS.contains(&extension)
}

pub fn vips_command_from_registry_value(value: &str) -> Result<String> {
    let config: MediaProcessingRegistryConfig = serde_json::from_str(value).map_err(|error| {
        AsterError::validation_error(format!(
            "media processing config must be valid JSON: {error}",
        ))
    })?;
    if config.version != MEDIA_PROCESSING_REGISTRY_VERSION {
        return Err(AsterError::validation_error(format!(
            "media processing config version must be {MEDIA_PROCESSING_REGISTRY_VERSION}",
        )));
    }

    let command = processor_config_for_kind(&config, MediaProcessorKind::VipsCli)
        .and_then(|processor| processor.config.command.as_deref())
        .unwrap_or(DEFAULT_VIPS_COMMAND);
    normalize_vips_command(command)
}

fn default_vips_extensions() -> Vec<String> {
    DEFAULT_VIPS_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_string())
        .collect()
}

pub fn public_thumbnail_support(runtime_config: &RuntimeConfig) -> PublicThumbnailSupport {
    let registry = media_processing_registry(runtime_config);
    let mut extensions = BTreeSet::new();

    for processor in registry
        .processors
        .iter()
        .filter(|processor| processor.enabled)
    {
        match processor.kind {
            MediaProcessorKind::Images => {
                extensions.extend(
                    BUILTIN_IMAGES_SUPPORTED_EXTENSIONS
                        .iter()
                        .map(|extension| (*extension).to_string()),
                );
            }
            MediaProcessorKind::VipsCli => {
                let command = processor
                    .config
                    .command
                    .as_deref()
                    .unwrap_or(DEFAULT_VIPS_COMMAND);
                if command_is_available(command) {
                    extensions.extend(processor.extensions.iter().cloned());
                }
            }
            MediaProcessorKind::StorageNative => {}
        }
    }

    PublicThumbnailSupport {
        version: PUBLIC_THUMBNAIL_SUPPORT_VERSION,
        extensions: extensions.into_iter().collect(),
    }
}

pub fn default_media_processing_registry() -> MediaProcessingRegistryConfig {
    MediaProcessingRegistryConfig {
        version: MEDIA_PROCESSING_REGISTRY_VERSION,
        processors: THUMBNAIL_PROCESSOR_PRIORITY
            .into_iter()
            .map(default_processor_config_for_kind)
            .collect(),
    }
}

pub fn default_media_processing_registry_json() -> String {
    serde_json::to_string_pretty(&default_media_processing_registry())
        .expect("default media processing registry should serialize")
}

pub fn normalize_media_processing_registry_config_value(value: &str) -> Result<String> {
    let mut config: MediaProcessingRegistryConfig =
        serde_json::from_str(value).map_err(|error| {
            AsterError::validation_error(format!(
                "media processing config must be valid JSON: {error}",
            ))
        })?;
    validate_media_processing_registry_config(&mut config, true)?;
    serde_json::to_string_pretty(&config).map_err(|error| {
        AsterError::internal_error(format!(
            "failed to serialize normalized media processing config: {error}",
        ))
    })
}

pub fn media_processing_registry(runtime_config: &RuntimeConfig) -> MediaProcessingRegistryConfig {
    let Some(raw) = runtime_config.get(MEDIA_PROCESSING_REGISTRY_JSON_KEY) else {
        return default_media_processing_registry();
    };

    match parse_media_processing_registry_config(&raw) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!("failed to parse media processing config: {error}");
            default_media_processing_registry()
        }
    }
}

pub fn thumbnail_processor_candidates(
    config: &MediaProcessingRegistryConfig,
    file_name: &str,
) -> Vec<MatchedThumbnailProcessor> {
    let extension = file_extension(file_name);
    let mut matched = Vec::new();

    for kind in THUMBNAIL_PROCESSOR_PRIORITY {
        let Some(processor) = processor_config_for_kind(config, kind) else {
            continue;
        };
        if !processor.enabled {
            continue;
        }

        if processor.kind == MediaProcessorKind::Images || processor.extensions.is_empty() {
            matched.push(MatchedThumbnailProcessor {
                processor: processor.clone(),
                match_kind: MediaProcessingMatchKind::Any,
            });
            continue;
        }

        let Some(extension) = extension.as_deref() else {
            continue;
        };
        if processor
            .extensions
            .iter()
            .any(|candidate| candidate == extension)
        {
            matched.push(MatchedThumbnailProcessor {
                processor: processor.clone(),
                match_kind: MediaProcessingMatchKind::Extension,
            });
        }
    }

    matched
}

pub fn file_extension(file_name: &str) -> Option<String> {
    Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
}

fn parse_media_processing_registry_config(value: &str) -> Result<MediaProcessingRegistryConfig> {
    let mut config: MediaProcessingRegistryConfig =
        serde_json::from_str(value).map_err(|error| {
            AsterError::validation_error(format!(
                "media processing config must be valid JSON: {error}",
            ))
        })?;
    validate_media_processing_registry_config(&mut config, false)?;
    Ok(config)
}

pub fn default_processor_config_for_kind(
    kind: MediaProcessorKind,
) -> MediaProcessingProcessorConfig {
    MediaProcessingProcessorConfig {
        kind,
        enabled: kind == MediaProcessorKind::Images,
        extensions: if kind == MediaProcessorKind::VipsCli {
            default_vips_extensions()
        } else {
            Vec::new()
        },
        config: if kind == MediaProcessorKind::VipsCli {
            MediaProcessingProcessorRuntimeConfig {
                command: Some(DEFAULT_VIPS_COMMAND.to_string()),
            }
        } else {
            MediaProcessingProcessorRuntimeConfig::default()
        },
    }
}

pub fn processor_config_for_kind(
    config: &MediaProcessingRegistryConfig,
    kind: MediaProcessorKind,
) -> Option<&MediaProcessingProcessorConfig> {
    config
        .processors
        .iter()
        .find(|processor| processor.kind == kind)
}

pub fn command_is_available(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }

    let command_path = Path::new(command);
    if command_path.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return command_path.is_file();
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(command);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

fn validate_media_processing_registry_config(
    config: &mut MediaProcessingRegistryConfig,
    validate_runtime_commands: bool,
) -> Result<()> {
    if config.version != MEDIA_PROCESSING_REGISTRY_VERSION {
        return Err(AsterError::validation_error(format!(
            "media processing config version must be {MEDIA_PROCESSING_REGISTRY_VERSION}",
        )));
    }

    let mut normalized = Vec::with_capacity(config.processors.len());
    let mut seen_kinds = HashSet::new();
    for mut processor in std::mem::take(&mut config.processors) {
        match processor.kind {
            MediaProcessorKind::Images => {
                processor.extensions.clear();
                processor.config = MediaProcessingProcessorRuntimeConfig::default();
            }
            MediaProcessorKind::VipsCli => {
                normalize_match_list(&mut processor.extensions)?;
                let command = processor
                    .config
                    .command
                    .as_deref()
                    .unwrap_or(DEFAULT_VIPS_COMMAND);
                let normalized_command = normalize_vips_command(command)?;
                if validate_runtime_commands
                    && processor.enabled
                    && !command_is_available(&normalized_command)
                {
                    return Err(AsterError::validation_error(format!(
                        "enabled vips_cli processor command '{normalized_command}' is not available",
                    )));
                }
                processor.config.command = Some(normalized_command);
            }
            MediaProcessorKind::StorageNative => {
                normalize_match_list(&mut processor.extensions)?;
                processor.config = MediaProcessingProcessorRuntimeConfig::default();
            }
        }

        let kind_key = processor.kind.as_str();
        if !seen_kinds.insert(kind_key) {
            return Err(AsterError::validation_error(format!(
                "duplicate media processing processor '{}'",
                kind_key
            )));
        }

        normalized.push(processor);
    }

    config.processors = THUMBNAIL_PROCESSOR_PRIORITY
        .into_iter()
        .map(|kind| {
            normalized
                .iter()
                .find(|processor| processor.kind == kind)
                .cloned()
                .unwrap_or_else(|| default_processor_config_for_kind(kind))
        })
        .collect();

    if !config.processors.iter().any(|processor| processor.enabled) {
        return Err(AsterError::validation_error(
            "media processing config must enable at least one processor",
        ));
    }

    Ok(())
}

fn normalize_match_list(items: &mut Vec<String>) -> Result<()> {
    let mut unique = BTreeSet::new();
    for item in std::mem::take(items) {
        unique.insert(normalize_extension(&item)?);
    }
    *items = unique.into_iter().collect();
    Ok(())
}

fn normalize_extension(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(
            "processor extension must not be empty",
        ));
    }
    Ok(trimmed.trim_start_matches('.').to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_IMAGES_SUPPORTED_EXTENSIONS, DEFAULT_VIPS_COMMAND, DEFAULT_VIPS_EXTENSIONS,
        MEDIA_PROCESSING_REGISTRY_JSON_KEY, MatchedThumbnailProcessor, MediaProcessingMatchKind,
        MediaProcessingProcessorConfig, MediaProcessingProcessorRuntimeConfig,
        MediaProcessingRegistryConfig, PublicThumbnailSupport, command_is_available,
        default_media_processing_registry, default_media_processing_registry_json, file_extension,
        media_processing_registry, normalize_media_processing_registry_config_value,
        normalize_vips_command, parse_media_processor_kind, processor_config_for_kind,
        public_thumbnail_support, thumbnail_processor_candidates, vips_command_from_registry_value,
    };
    use crate::config::RuntimeConfig;
    use crate::entities::system_config;
    use crate::types::{MediaProcessorKind, SystemConfigSource, SystemConfigValueType};
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 0,
            key: key.to_string(),
            value: value.to_string(),
            value_type: SystemConfigValueType::String,
            requires_restart: false,
            is_sensitive: false,
            source: SystemConfigSource::System,
            namespace: String::new(),
            category: "test".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn parse_media_processor_kind_understands_known_values() {
        assert_eq!(
            parse_media_processor_kind(" images "),
            Some(MediaProcessorKind::Images)
        );
        assert_eq!(
            parse_media_processor_kind("vips_cli"),
            Some(MediaProcessorKind::VipsCli)
        );
        assert_eq!(
            parse_media_processor_kind("storage_native"),
            Some(MediaProcessorKind::StorageNative)
        );
        assert_eq!(parse_media_processor_kind("nope"), None);
    }

    #[test]
    fn normalize_vips_command_trims_and_defaults() {
        assert_eq!(
            normalize_vips_command("  /usr/bin/vips  ").unwrap(),
            "/usr/bin/vips"
        );
        assert_eq!(normalize_vips_command(" ").unwrap(), DEFAULT_VIPS_COMMAND);
    }

    #[test]
    fn builtin_images_supports_known_extensions() {
        for extension in BUILTIN_IMAGES_SUPPORTED_EXTENSIONS {
            assert!(super::builtin_images_supports_extension(extension));
        }
        assert!(!super::builtin_images_supports_extension("heic"));
    }

    #[test]
    fn vips_command_from_registry_value_prefers_draft_command() {
        let command = vips_command_from_registry_value(
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": false,
                        "config": {
                            "command": "  /usr/local/bin/vips  "
                        }
                    },
                    {
                        "kind": "images",
                        "enabled": true
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(command, "/usr/local/bin/vips");
    }

    #[test]
    fn command_is_available_rejects_blank_command() {
        assert!(!command_is_available(""));
        assert!(!command_is_available("   "));
    }

    #[test]
    fn default_registry_includes_known_processors_in_fixed_order() {
        let config = default_media_processing_registry();
        assert_eq!(config.version, 1);
        assert_eq!(
            config.processors,
            vec![
                MediaProcessingProcessorConfig {
                    kind: MediaProcessorKind::VipsCli,
                    enabled: false,
                    extensions: DEFAULT_VIPS_EXTENSIONS
                        .iter()
                        .map(|extension| (*extension).to_string())
                        .collect(),
                    config: MediaProcessingProcessorRuntimeConfig {
                        command: Some(DEFAULT_VIPS_COMMAND.to_string()),
                    },
                },
                MediaProcessingProcessorConfig {
                    kind: MediaProcessorKind::Images,
                    enabled: true,
                    extensions: vec![],
                    config: MediaProcessingProcessorRuntimeConfig::default(),
                },
            ]
        );

        let json = default_media_processing_registry_json();
        assert!(json.contains("\"vips_cli\""));
        assert!(json.contains("\"images\""));
        assert!(json.contains("\"heic\""));
        assert!(json.contains("\"avif\""));
    }

    #[test]
    fn public_thumbnail_support_exposes_enabled_processor_capabilities() {
        let mut runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            MEDIA_PROCESSING_REGISTRY_JSON_KEY,
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true,
                        "extensions": ["HEIC", ".avif"],
                        "config": {
                            "command": "/bin/sh"
                        }
                    },
                    {
                        "kind": "images",
                        "enabled": false
                    }
                ]
            }"#,
        ));

        assert_eq!(
            public_thumbnail_support(&runtime_config),
            PublicThumbnailSupport {
                version: 1,
                extensions: vec!["avif".to_string(), "heic".to_string()],
            }
        );
    }

    #[test]
    fn public_thumbnail_support_keeps_builtin_extensions_when_images_are_enabled() {
        let support = public_thumbnail_support(&RuntimeConfig::new());
        let expected = BUILTIN_IMAGES_SUPPORTED_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(support.version, 1);
        assert_eq!(support.extensions, expected);
    }

    #[test]
    fn normalize_media_processing_registry_merges_missing_processors_with_defaults() {
        let normalized = normalize_media_processing_registry_config_value(
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true,
                        "extensions": ["HEIC", ".heif", "heic"],
                        "config": {
                            "command": "  /bin/sh  "
                        }
                    }
                ]
            }"#,
        )
        .unwrap();

        let parsed: MediaProcessingRegistryConfig = serde_json::from_str(&normalized).unwrap();
        assert_eq!(parsed.processors.len(), 2);
        assert_eq!(
            parsed.processors[0],
            MediaProcessingProcessorConfig {
                kind: MediaProcessorKind::VipsCli,
                enabled: true,
                extensions: vec!["heic".to_string(), "heif".to_string()],
                config: MediaProcessingProcessorRuntimeConfig {
                    command: Some("/bin/sh".to_string()),
                },
            }
        );
        assert_eq!(parsed.processors[1].kind, MediaProcessorKind::Images);
        assert!(parsed.processors[1].enabled);
    }

    #[test]
    fn normalize_media_processing_registry_requires_one_enabled_processor() {
        let error = normalize_media_processing_registry_config_value(
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "images",
                        "enabled": false
                    }
                ]
            }"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("enable at least one processor"));
    }

    #[test]
    fn normalize_media_processing_registry_rejects_unavailable_enabled_vips_command() {
        let error = normalize_media_processing_registry_config_value(
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true,
                        "config": {
                            "command": "definitely-missing-vips-cli"
                        }
                    },
                    {
                        "kind": "images",
                        "enabled": true
                    }
                ]
            }"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("not available"));
    }

    #[test]
    fn thumbnail_processor_candidates_use_fixed_processor_priority() {
        let config = MediaProcessingRegistryConfig {
            version: 1,
            processors: vec![
                MediaProcessingProcessorConfig {
                    kind: MediaProcessorKind::VipsCli,
                    enabled: true,
                    extensions: vec!["heic".to_string()],
                    config: MediaProcessingProcessorRuntimeConfig {
                        command: Some(DEFAULT_VIPS_COMMAND.to_string()),
                    },
                },
                MediaProcessingProcessorConfig {
                    kind: MediaProcessorKind::Images,
                    enabled: true,
                    extensions: vec![],
                    config: MediaProcessingProcessorRuntimeConfig::default(),
                },
            ],
        };

        assert_eq!(
            thumbnail_processor_candidates(&config, "photo.heic"),
            vec![
                MatchedThumbnailProcessor {
                    processor: MediaProcessingProcessorConfig {
                        kind: MediaProcessorKind::VipsCli,
                        enabled: true,
                        extensions: vec!["heic".to_string()],
                        config: MediaProcessingProcessorRuntimeConfig {
                            command: Some(DEFAULT_VIPS_COMMAND.to_string()),
                        },
                    },
                    match_kind: MediaProcessingMatchKind::Extension,
                },
                MatchedThumbnailProcessor {
                    processor: MediaProcessingProcessorConfig {
                        kind: MediaProcessorKind::Images,
                        enabled: true,
                        extensions: vec![],
                        config: MediaProcessingProcessorRuntimeConfig::default(),
                    },
                    match_kind: MediaProcessingMatchKind::Any,
                },
            ]
        );
        assert_eq!(
            thumbnail_processor_candidates(&config, "photo.png"),
            vec![MatchedThumbnailProcessor {
                processor: MediaProcessingProcessorConfig {
                    kind: MediaProcessorKind::Images,
                    enabled: true,
                    extensions: vec![],
                    config: MediaProcessingProcessorRuntimeConfig::default(),
                },
                match_kind: MediaProcessingMatchKind::Any,
            },]
        );
    }

    #[test]
    fn file_extension_normalizes_suffixes() {
        assert_eq!(file_extension("photo.HEIC"), Some("heic".to_string()));
        assert_eq!(file_extension("archive"), None);
    }

    #[test]
    fn runtime_readers_fall_back_to_defaults() {
        let runtime_config = RuntimeConfig::new();
        assert_eq!(
            media_processing_registry(&runtime_config),
            default_media_processing_registry()
        );
    }

    #[test]
    fn runtime_readers_use_applied_values() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            "media_processing_registry_json",
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true
                    }
                ]
            }"#,
        ));

        assert_eq!(
            media_processing_registry(&runtime_config).processors[0].kind,
            MediaProcessorKind::VipsCli
        );
    }

    #[test]
    fn runtime_readers_keep_vips_processor_even_when_command_is_unavailable() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            "media_processing_registry_json",
            r#"{
                "version": 1,
                "processors": [
                    {
                        "kind": "vips_cli",
                        "enabled": true,
                        "config": {
                            "command": "definitely-missing-vips-cli"
                        }
                    },
                    {
                        "kind": "images",
                        "enabled": true
                    }
                ]
            }"#,
        ));

        let config = media_processing_registry(&runtime_config);
        let processor = processor_config_for_kind(&config, MediaProcessorKind::VipsCli)
            .expect("vips_cli processor should exist");
        assert!(processor.enabled);
        assert_eq!(
            processor.config.command.as_deref(),
            Some("definitely-missing-vips-cli")
        );
    }
}
