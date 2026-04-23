use std::collections::{BTreeSet, HashSet};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::RuntimeConfig;
use crate::errors::{AsterError, Result};
use crate::types::MediaProcessorKind;

use super::types::{
    BUILTIN_IMAGES_SUPPORTED_EXTENSIONS, DEFAULT_FFMPEG_COMMAND, DEFAULT_FFMPEG_EXTENSIONS,
    DEFAULT_VIPS_COMMAND, DEFAULT_VIPS_EXTENSIONS, MEDIA_PROCESSING_REGISTRY_VERSION,
    MatchedMediaProcessor, MediaProcessingMatchKind, MediaProcessingProcessorConfig,
    MediaProcessingProcessorRuntimeConfig, MediaProcessingRegistryConfig,
    PUBLIC_THUMBNAIL_SUPPORT_VERSION, PublicThumbnailSupport,
};
use crate::config::definitions::MEDIA_PROCESSING_REGISTRY_JSON_KEY;

const THUMBNAIL_PROCESSOR_PRIORITY: [MediaProcessorKind; 3] = [
    MediaProcessorKind::VipsCli,
    MediaProcessorKind::FfmpegCli,
    MediaProcessorKind::Images,
];

pub fn parse_media_processor_kind(value: &str) -> Option<MediaProcessorKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "images" => Some(MediaProcessorKind::Images),
        "vips_cli" => Some(MediaProcessorKind::VipsCli),
        "ffmpeg_cli" => Some(MediaProcessorKind::FfmpegCli),
        "storage_native" => Some(MediaProcessorKind::StorageNative),
        _ => None,
    }
}

pub fn normalize_vips_command(value: &str) -> Result<String> {
    normalize_processor_command(value, DEFAULT_VIPS_COMMAND, "vips command")
}

pub fn normalize_ffmpeg_command(value: &str) -> Result<String> {
    normalize_processor_command(value, DEFAULT_FFMPEG_COMMAND, "ffmpeg command")
}

fn normalize_processor_command(value: &str, default_command: &str, label: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(default_command.to_string());
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(AsterError::validation_error(format!(
            "{label} cannot contain control characters"
        )));
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

pub fn ffmpeg_command_from_registry_value(value: &str) -> Result<String> {
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

    let command = processor_config_for_kind(&config, MediaProcessorKind::FfmpegCli)
        .and_then(|processor| processor.config.command.as_deref())
        .unwrap_or(DEFAULT_FFMPEG_COMMAND);
    normalize_ffmpeg_command(command)
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
            MediaProcessorKind::FfmpegCli => {
                let command = processor
                    .config
                    .command
                    .as_deref()
                    .unwrap_or(DEFAULT_FFMPEG_COMMAND);
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

pub fn processor_candidates_for_file_name(
    config: &MediaProcessingRegistryConfig,
    file_name: &str,
) -> Vec<MatchedMediaProcessor> {
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
            matched.push(MatchedMediaProcessor {
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
            matched.push(MatchedMediaProcessor {
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

pub fn default_processor_config_for_kind(
    kind: MediaProcessorKind,
) -> MediaProcessingProcessorConfig {
    MediaProcessingProcessorConfig {
        kind,
        enabled: kind == MediaProcessorKind::Images,
        extensions: match kind {
            MediaProcessorKind::VipsCli => DEFAULT_VIPS_EXTENSIONS
                .iter()
                .map(|extension| (*extension).to_string())
                .collect(),
            MediaProcessorKind::FfmpegCli => DEFAULT_FFMPEG_EXTENSIONS
                .iter()
                .map(|extension| (*extension).to_string())
                .collect(),
            _ => Vec::new(),
        },
        config: match kind {
            MediaProcessorKind::VipsCli => MediaProcessingProcessorRuntimeConfig {
                command: Some(DEFAULT_VIPS_COMMAND.to_string()),
            },
            MediaProcessorKind::FfmpegCli => MediaProcessingProcessorRuntimeConfig {
                command: Some(DEFAULT_FFMPEG_COMMAND.to_string()),
            },
            _ => MediaProcessingProcessorRuntimeConfig::default(),
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
        return is_executable_file(command_path);
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(command);
                is_executable_file(&candidate)
            })
        })
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
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
            MediaProcessorKind::FfmpegCli => {
                normalize_match_list(&mut processor.extensions)?;
                let command = processor
                    .config
                    .command
                    .as_deref()
                    .unwrap_or(DEFAULT_FFMPEG_COMMAND);
                let normalized_command = normalize_ffmpeg_command(command)?;
                if validate_runtime_commands
                    && processor.enabled
                    && !command_is_available(&normalized_command)
                {
                    return Err(AsterError::validation_error(format!(
                        "enabled ffmpeg_cli processor command '{normalized_command}' is not available",
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
