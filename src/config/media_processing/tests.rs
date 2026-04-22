use crate::config::RuntimeConfig;
use crate::entities::system_config;
use crate::types::{MediaProcessorKind, SystemConfigSource, SystemConfigValueType};
use chrono::Utc;

use super::{
    BUILTIN_IMAGES_SUPPORTED_EXTENSIONS, DEFAULT_VIPS_COMMAND, DEFAULT_VIPS_EXTENSIONS,
    MEDIA_PROCESSING_REGISTRY_JSON_KEY, MatchedMediaProcessor, MediaProcessingMatchKind,
    MediaProcessingProcessorConfig, MediaProcessingProcessorRuntimeConfig,
    MediaProcessingRegistryConfig, PublicThumbnailSupport, command_is_available,
    default_media_processing_registry, default_media_processing_registry_json, file_extension,
    media_processing_registry, normalize_media_processing_registry_config_value,
    normalize_vips_command, parse_media_processor_kind, processor_candidates_for_file_name,
    processor_config_for_kind, public_thumbnail_support, vips_command_from_registry_value,
};

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
    let runtime_config = RuntimeConfig::new();
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
fn processor_candidates_for_file_name_use_fixed_processor_priority() {
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
        processor_candidates_for_file_name(&config, "photo.heic"),
        vec![
            MatchedMediaProcessor {
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
            MatchedMediaProcessor {
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
        processor_candidates_for_file_name(&config, "photo.png"),
        vec![MatchedMediaProcessor {
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
