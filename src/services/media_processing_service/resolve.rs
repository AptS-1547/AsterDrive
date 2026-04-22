use crate::config::media_processing as media_processing_config;
use crate::entities::{file_blob, storage_policy};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;
use crate::types::{MediaProcessorKind, parse_storage_policy_options};

use super::shared::{MediaOperation, ResolvedMediaProcessor, ThumbnailContext};

pub(crate) fn resolve_thumbnail_processor_for_blob(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
) -> Result<ResolvedMediaProcessor> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    resolve_thumbnail_processor_for_policy(state, &policy, file_name)
}

pub(super) fn build_thumbnail_context(
    state: &PrimaryAppState,
    blob: &file_blob::Model,
    file_name: &str,
) -> Result<ThumbnailContext> {
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let processor = resolve_thumbnail_processor_for_policy(state, &policy, file_name)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    Ok(ThumbnailContext { driver, processor })
}

pub(super) fn build_thumbnail_context_with_processor(
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
    let processor = resolved_media_processor_from_config(&processor_config);
    if let Some(reason) = processor_unavailable_reason(
        &processor_config,
        Some(source_file_name),
        Some((state, policy)),
    )? {
        return Err(AsterError::precondition_failed(reason));
    }

    let driver = state.driver_registry.get_driver(policy)?;
    let source_extension = media_processing_config::file_extension(source_file_name);
    tracing::debug!(
        operation = MediaOperation::Thumbnail.as_str(),
        policy_id = policy.id,
        processor = processor_kind.as_str(),
        selection_source = "task_payload",
        source_file_name,
        source_extension = source_extension.as_deref().unwrap_or(""),
        "built thumbnail context with explicit processor"
    );
    Ok(ThumbnailContext { driver, processor })
}

pub(super) fn resolve_avatar_processor(
    runtime_config: &crate::config::RuntimeConfig,
    file_name: &str,
) -> Result<ResolvedMediaProcessor> {
    let candidates = collect_global_processor_candidates(runtime_config, file_name);
    resolve_media_processor_from_candidates(
        MediaOperation::Avatar,
        file_name,
        None,
        None,
        candidates,
    )
}

fn resolve_thumbnail_processor_for_policy(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
    file_name: &str,
) -> Result<ResolvedMediaProcessor> {
    let candidates =
        collect_thumbnail_processor_candidates(&state.runtime_config, policy, file_name);
    resolve_media_processor_from_candidates(
        MediaOperation::Thumbnail,
        file_name,
        Some(policy.id),
        Some((state, policy)),
        candidates,
    )
}

fn resolved_media_processor_from_config(
    processor: &media_processing_config::MediaProcessingProcessorConfig,
) -> ResolvedMediaProcessor {
    match processor.kind {
        MediaProcessorKind::VipsCli => ResolvedMediaProcessor::with_vips_command(
            processor
                .config
                .command
                .clone()
                .unwrap_or_else(|| media_processing_config::DEFAULT_VIPS_COMMAND.to_string()),
        ),
        _ => ResolvedMediaProcessor::new(processor.kind),
    }
}

fn processor_unavailable_reason(
    processor: &media_processing_config::MediaProcessingProcessorConfig,
    source_file_name: Option<&str>,
    storage_policy_context: Option<(&PrimaryAppState, &storage_policy::Model)>,
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
                    "vips CLI command '{command}' is not available"
                )));
            }
            Ok(None)
        }
        MediaProcessorKind::StorageNative => {
            let Some((state, policy)) = storage_policy_context else {
                return Ok(Some(
                    "storage-native media processor requires storage policy context".to_string(),
                ));
            };
            storage_native_processor_unavailable_reason(state, policy)
        }
    }
}

fn storage_native_processor_unavailable_reason(
    state: &PrimaryAppState,
    policy: &storage_policy::Model,
) -> Result<Option<String>> {
    let driver = state.driver_registry.get_driver(policy)?;
    if driver.as_native_thumbnail().is_none() {
        return Ok(Some(format!(
            "storage policy #{} does not expose storage-native thumbnail processing",
            policy.id
        )));
    }
    Ok(None)
}

fn collect_global_processor_candidates(
    runtime_config: &crate::config::RuntimeConfig,
    file_name: &str,
) -> Vec<media_processing_config::MatchedMediaProcessor> {
    let registry = media_processing_config::media_processing_registry(runtime_config);
    media_processing_config::processor_candidates_for_file_name(&registry, file_name)
}

fn collect_thumbnail_processor_candidates(
    runtime_config: &crate::config::RuntimeConfig,
    policy: &storage_policy::Model,
    file_name: &str,
) -> Vec<media_processing_config::MatchedMediaProcessor> {
    let source_extension = media_processing_config::file_extension(file_name);
    let policy_options = parse_storage_policy_options(policy.options.as_ref());
    let mut candidates = Vec::new();

    if policy_options.thumbnail_processor == Some(MediaProcessorKind::StorageNative) {
        if !policy_options.storage_native_thumbnail_matches_file_name(file_name) {
            tracing::debug!(
                operation = MediaOperation::Thumbnail.as_str(),
                policy_id = policy.id,
                file_name,
                source_extension = source_extension.as_deref().unwrap_or(""),
                processor = MediaProcessorKind::StorageNative.as_str(),
                processor_match =
                    media_processing_config::MediaProcessingMatchKind::Policy.as_str(),
                skip_reason = "policy thumbnail extension binding did not match source file",
                "skipped unmatched policy-native media processor"
            );
        } else {
            candidates.push(media_processing_config::MatchedMediaProcessor {
                processor: media_processing_config::default_processor_config_for_kind(
                    MediaProcessorKind::StorageNative,
                ),
                match_kind: media_processing_config::MediaProcessingMatchKind::Policy,
            });
        }
    }

    candidates.extend(collect_global_processor_candidates(
        runtime_config,
        file_name,
    ));
    candidates
}

fn resolve_media_processor_from_candidates(
    operation: MediaOperation,
    file_name: &str,
    policy_id: Option<i64>,
    storage_policy_context: Option<(&PrimaryAppState, &storage_policy::Model)>,
    candidates: Vec<media_processing_config::MatchedMediaProcessor>,
) -> Result<ResolvedMediaProcessor> {
    let source_extension = media_processing_config::file_extension(file_name);
    if candidates.is_empty() {
        return Err(AsterError::precondition_failed(format!(
            "no enabled {} processor matched '{file_name}'",
            operation.as_str()
        )));
    }

    let mut last_unavailable_reason = None;
    for candidate in candidates {
        let unavailable_reason = processor_unavailable_reason(
            &candidate.processor,
            Some(file_name),
            storage_policy_context,
        )?;
        if let Some(reason) = unavailable_reason {
            tracing::debug!(
                operation = operation.as_str(),
                policy_id = ?policy_id,
                file_name,
                source_extension = source_extension.as_deref().unwrap_or(""),
                processor = candidate.processor.kind.as_str(),
                processor_match = candidate.match_kind.as_str(),
                skip_reason = %reason,
                "skipped unavailable media processor"
            );
            last_unavailable_reason = Some(reason);
            continue;
        }

        tracing::debug!(
            operation = operation.as_str(),
            policy_id = ?policy_id,
            file_name,
            source_extension = source_extension.as_deref().unwrap_or(""),
            processor = candidate.processor.kind.as_str(),
            processor_match = candidate.match_kind.as_str(),
            "resolved media processor"
        );
        return Ok(resolved_media_processor_from_config(&candidate.processor));
    }

    let reason = last_unavailable_reason.unwrap_or_else(|| {
        format!(
            "no available {} processor matched '{file_name}'",
            operation.as_str()
        )
    });
    Err(AsterError::precondition_failed(reason))
}
