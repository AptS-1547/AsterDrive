mod import;
mod staging;

use std::path::Path;

use chrono::Utc;

use crate::entities::{background_task, file};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::{
    storage_change_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::types::BackgroundTaskKind;

use super::super::steps::{
    TASK_STEP_DOWNLOAD_SOURCE, TASK_STEP_IMPORT_RESULT, TASK_STEP_WAITING, parse_task_steps_json,
    set_task_step_active, set_task_step_succeeded,
};
use super::super::types::{
    ArchiveExtractTaskPayload, ArchiveExtractTaskResult, CreateArchiveExtractTaskParams,
    parse_task_payload, serialize_task_result,
};
use super::super::{
    TaskLeaseGuard, cleanup_task_temp_dir_for_task, create_task_record, mark_task_progress,
    mark_task_succeeded, prepare_task_temp_dir, task_scope,
};
use super::common::{build_folder_display_path, create_unique_folder_in_scope};
use import::materialize_archive_extract_stage;
use staging::{download_file_to_temp, stage_zip_archive_for_extract};

pub(crate) async fn create_archive_extract_task_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_id: i64,
    params: CreateArchiveExtractTaskParams,
) -> Result<super::super::TaskInfo> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    let source_file = workspace_storage_service::verify_file_access(state, scope, file_id).await?;
    workspace_storage_service::ensure_active_file_scope(&source_file, scope)?;
    ensure_extract_source_supported(&source_file)?;

    if let Some(target_folder_id) = params.target_folder_id {
        workspace_storage_service::verify_folder_access(state, scope, target_folder_id).await?;
    }

    let payload = ArchiveExtractTaskPayload {
        file_id: source_file.id,
        source_file_name: source_file.name.clone(),
        target_folder_id: params.target_folder_id.or(source_file.folder_id),
        output_folder_name: resolve_extract_output_folder_name(
            params.output_folder_name.as_ref(),
            &source_file.name,
        )?,
    };
    let display_name = format!("Extract {}", source_file.name);
    let task = create_task_record(
        state,
        scope,
        BackgroundTaskKind::ArchiveExtract,
        &display_name,
        &payload,
    )
    .await?;
    super::super::get_task_in_scope(state, scope, task.id).await
}

pub(super) async fn process_archive_extract_task(
    state: &AppState,
    task: &background_task::Model,
    lease_guard: TaskLeaseGuard,
) -> Result<()> {
    let scope = task_scope(task)?;
    let payload: ArchiveExtractTaskPayload = parse_task_payload(task)?;
    let mut steps =
        parse_task_steps_json(task.steps_json.as_ref().map(|raw| raw.as_ref()), task.kind)?;
    set_task_step_succeeded(
        &mut steps,
        TASK_STEP_WAITING,
        Some("Worker claimed task"),
        None,
    )?;
    let source_file =
        workspace_storage_service::verify_file_access(state, scope, payload.file_id).await?;
    workspace_storage_service::ensure_active_file_scope(&source_file, scope)?;
    ensure_extract_source_supported(&source_file)?;
    if let Some(target_folder_id) = payload.target_folder_id {
        workspace_storage_service::verify_folder_access(state, scope, target_folder_id).await?;
    }

    set_task_step_active(
        &mut steps,
        TASK_STEP_DOWNLOAD_SOURCE,
        Some("Downloading source archive"),
        None,
    )?;
    mark_task_progress(
        state,
        &lease_guard,
        0,
        0,
        Some("Downloading source archive"),
        &steps,
    )
    .await?;
    let task_temp_dir = prepare_task_temp_dir(state, lease_guard.lease()).await?;
    let task_temp_path = Path::new(&task_temp_dir);
    let source_archive_path = task_temp_path.join("source.zip");
    let stage_root = task_temp_path.join("extract");
    tokio::fs::create_dir_all(&stage_root)
        .await
        .map_aster_err_ctx(
            "create archive extract staging dir",
            AsterError::storage_driver_error,
        )?;
    download_file_to_temp(state, &source_file, &source_archive_path).await?;
    set_task_step_succeeded(
        &mut steps,
        TASK_STEP_DOWNLOAD_SOURCE,
        Some("Source archive downloaded"),
        None,
    )?;
    let steps_for_worker = steps.clone();

    let db = state.db.clone();
    let handle = tokio::runtime::Handle::current();
    let lease_guard_for_worker = lease_guard.clone();
    let source_archive_path_string = source_archive_path.to_string_lossy().to_string();
    let stage_root_string = stage_root.to_string_lossy().to_string();
    let (staged, mut steps) = tokio::task::spawn_blocking(move || {
        let mut steps = steps_for_worker;
        let staged = stage_zip_archive_for_extract(
            &handle,
            &db,
            &lease_guard_for_worker,
            &source_archive_path_string,
            &stage_root_string,
            &mut steps,
        )?;
        Ok::<_, AsterError>((staged, steps))
    })
    .await
    .map_err(|error| {
        AsterError::internal_error(format!("archive extract worker failed: {error}"))
    })??;

    let created_root = create_unique_folder_in_scope(
        state,
        scope,
        payload.target_folder_id,
        &payload.output_folder_name,
    )
    .await?;
    storage_change_service::publish(
        state,
        storage_change_service::StorageChangeEvent::new(
            storage_change_service::StorageChangeKind::FolderCreated,
            scope,
            vec![],
            vec![created_root.id],
            vec![created_root.parent_id],
        ),
    );

    set_task_step_active(
        &mut steps,
        TASK_STEP_IMPORT_RESULT,
        Some("Importing extracted files"),
        Some((0, staged.total_bytes)),
    )?;
    mark_task_progress(
        state,
        &lease_guard,
        staged.total_bytes,
        staged.total_progress,
        Some("Importing extracted files"),
        &steps,
    )
    .await?;
    materialize_archive_extract_stage(
        state,
        &lease_guard,
        scope,
        &stage_root,
        staged.total_bytes,
        &created_root,
        &mut steps,
    )
    .await?;
    cleanup_task_temp_dir_for_task(state, task.id).await?;
    set_task_step_succeeded(
        &mut steps,
        TASK_STEP_IMPORT_RESULT,
        Some(&format!("Imported into {}", created_root.name)),
        Some((staged.total_bytes, staged.total_bytes)),
    )?;

    let result = ArchiveExtractTaskResult {
        target_folder_id: created_root.id,
        target_folder_name: created_root.name.clone(),
        target_path: build_folder_display_path(&state.db, created_root.id).await?,
        extracted_file_count: staged.file_count,
        extracted_folder_count: staged.directory_count,
    };
    let result_json = serialize_task_result(&result)?;
    let progress_total = staged.total_progress;
    mark_task_succeeded(
        state,
        &lease_guard,
        Some(&result_json),
        progress_total,
        progress_total,
        Some(&format!("Extracted to {}", created_root.name)),
        &steps,
    )
    .await
}

fn ensure_extract_source_supported(source_file: &file::Model) -> Result<()> {
    if source_file.name.to_ascii_lowercase().ends_with(".zip") {
        Ok(())
    } else {
        Err(AsterError::validation_error(
            "online extract currently supports .zip files only",
        ))
    }
}

fn resolve_extract_output_folder_name(
    output_folder_name: Option<&String>,
    source_file_name: &str,
) -> Result<String> {
    let candidate = match output_folder_name.map(|value| value.trim()) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => default_extract_output_folder_name(source_file_name),
    };
    crate::utils::validate_name(&candidate)?;
    Ok(candidate)
}

fn default_extract_output_folder_name(source_file_name: &str) -> String {
    if let Some(stripped) = strip_zip_extension(source_file_name)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }
    format!("extracted-{}", Utc::now().format("%Y%m%d-%H%M%S"))
}

fn strip_zip_extension(name: &str) -> Option<&str> {
    if name.to_ascii_lowercase().ends_with(".zip") && name.len() > 4 {
        Some(&name[..name.len() - 4])
    } else {
        None
    }
}
