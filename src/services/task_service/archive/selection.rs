use std::collections::{HashMap, HashSet};

use actix_web::HttpResponse;
use chrono::Utc;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::{
    batch_service, folder_service,
    workspace_storage_service::{self, WorkspaceStorageScope},
};

use super::super::types::CreateArchiveTaskParams;
use super::common::{ArchiveEntry, is_client_disconnect_error_text, write_archive_to_sink};

pub(crate) struct PreparedArchiveDownload {
    pub file_ids: Vec<i64>,
    pub folder_ids: Vec<i64>,
    pub archive_name: String,
}

pub(super) struct ResolvedArchiveDownload {
    pub(super) selection: batch_service::NormalizedSelection,
    pub(super) archive_name: String,
}

pub(crate) async fn stream_archive_download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    params: CreateArchiveTaskParams,
) -> Result<HttpResponse> {
    let resolved = resolve_archive_download_in_scope(state, scope, &params).await?;
    let archive_name = resolved.archive_name.clone();
    let (entries, total_bytes) =
        collect_archive_entries_from_selection_in_scope(state, scope, &resolved.selection).await?;

    let (reader, writer) = tokio::io::duplex(64 * 1024);
    let handle = tokio::runtime::Handle::current();
    let db = state.db.clone();
    let driver_registry = state.driver_registry.clone();
    let policy_snapshot = state.policy_snapshot.clone();
    let archive_name_for_worker = archive_name.clone();

    drop(tokio::task::spawn_blocking(move || {
        let writer = tokio_util::io::SyncIoBridge::new(writer);
        let writer = std::io::BufWriter::new(writer);
        if let Err(error) = write_archive_to_sink(
            &handle,
            &db,
            driver_registry.as_ref(),
            policy_snapshot.as_ref(),
            None,
            entries,
            total_bytes,
            writer,
            |_, _| Ok(()),
        ) {
            let error_text = error.to_string();
            if is_client_disconnect_error_text(&error_text) {
                tracing::info!(
                    archive_name = %archive_name_for_worker,
                    "archive download stream stopped after client disconnected"
                );
            } else {
                tracing::warn!(
                    archive_name = %archive_name_for_worker,
                    error = %error_text,
                    "archive download stream failed"
                );
            }
        }
    }));

    let reader_stream = tokio_util::io::ReaderStream::with_capacity(reader, 64 * 1024);

    Ok(HttpResponse::Ok()
        .content_type("application/zip")
        .insert_header((
            "Content-Disposition",
            format!(r#"attachment; filename="{}""#, archive_name),
        ))
        .insert_header(("Content-Encoding", "identity"))
        .streaming(reader_stream))
}

pub(crate) async fn prepare_archive_download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    params: &CreateArchiveTaskParams,
) -> Result<PreparedArchiveDownload> {
    let resolved = resolve_archive_download_in_scope(state, scope, params).await?;
    Ok(PreparedArchiveDownload {
        file_ids: resolved.selection.file_ids,
        folder_ids: resolved.selection.folder_ids,
        archive_name: resolved.archive_name,
    })
}

pub(super) async fn resolve_archive_download_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    params: &CreateArchiveTaskParams,
) -> Result<ResolvedArchiveDownload> {
    ensure_archive_selection_request_in_scope(state, scope, &params.file_ids, &params.folder_ids)
        .await?;
    let selection = batch_service::load_normalized_selection_in_scope(
        state,
        scope,
        &params.file_ids,
        &params.folder_ids,
    )
    .await?;
    ensure_archive_selection_active(scope, &selection)?;
    let archive_name = resolve_archive_name(&params.archive_name, &selection)?;

    Ok(ResolvedArchiveDownload {
        selection,
        archive_name,
    })
}

async fn ensure_archive_selection_request_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    file_ids: &[i64],
    folder_ids: &[i64],
) -> Result<()> {
    workspace_storage_service::require_scope_access(state, scope).await?;
    batch_service::validate_batch_ids(file_ids, folder_ids)?;

    let file_map: HashMap<i64, file::Model> = file_repo::find_by_ids(&state.db, file_ids)
        .await?
        .into_iter()
        .map(|file| (file.id, file))
        .collect();
    for &file_id in file_ids {
        let file = file_map
            .get(&file_id)
            .ok_or_else(|| AsterError::file_not_found(format!("file #{file_id}")))?;
        workspace_storage_service::ensure_active_file_scope(file, scope)?;
    }

    let folder_map: HashMap<i64, folder::Model> = folder_repo::find_by_ids(&state.db, folder_ids)
        .await?
        .into_iter()
        .map(|folder| (folder.id, folder))
        .collect();
    for &folder_id in folder_ids {
        let folder = folder_map
            .get(&folder_id)
            .ok_or_else(|| AsterError::folder_not_found(format!("folder #{folder_id}")))?;
        workspace_storage_service::ensure_active_folder_scope(folder, scope)?;
    }

    Ok(())
}

pub(super) fn ensure_archive_selection_active(
    scope: WorkspaceStorageScope,
    selection: &batch_service::NormalizedSelection,
) -> Result<()> {
    for &file_id in &selection.file_ids {
        let file = selection
            .file_map
            .get(&file_id)
            .ok_or_else(|| AsterError::file_not_found(format!("file #{file_id}")))?;
        workspace_storage_service::ensure_active_file_scope(file, scope)?;
    }

    for &folder_id in &selection.folder_ids {
        let folder = selection
            .folder_map
            .get(&folder_id)
            .ok_or_else(|| AsterError::folder_not_found(format!("folder #{folder_id}")))?;
        workspace_storage_service::ensure_active_folder_scope(folder, scope)?;
    }

    Ok(())
}

pub(super) async fn collect_archive_entries_from_selection_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    selection: &batch_service::NormalizedSelection,
) -> Result<(Vec<ArchiveEntry>, i64)> {
    let mut entries = Vec::new();
    let mut total_bytes = 0_i64;
    let mut reserved_root_names = HashSet::new();

    for &file_id in &selection.file_ids {
        let file = selection
            .file_map
            .get(&file_id)
            .ok_or_else(|| AsterError::file_not_found(format!("file #{file_id}")))?;
        workspace_storage_service::ensure_active_file_scope(file, scope)?;
        let entry_path = batch_service::reserve_unique_name(&mut reserved_root_names, &file.name);
        total_bytes = total_bytes
            .checked_add(file.size)
            .ok_or_else(|| AsterError::internal_error("archive size overflow"))?;
        entries.push(ArchiveEntry::File {
            file: file.clone(),
            entry_path,
        });
    }

    for &folder_id in &selection.folder_ids {
        let folder = selection
            .folder_map
            .get(&folder_id)
            .ok_or_else(|| AsterError::folder_not_found(format!("folder #{folder_id}")))?;
        workspace_storage_service::ensure_active_folder_scope(folder, scope)?;
        let archive_root =
            batch_service::reserve_unique_name(&mut reserved_root_names, &folder.name);

        let (tree_files, tree_folder_ids) =
            folder_service::collect_folder_tree_in_scope(&state.db, scope, folder_id, false)
                .await?;
        let folder_paths = folder_service::build_folder_paths(&state.db, &tree_folder_ids).await?;
        let root_path = folder_paths
            .get(&folder_id)
            .cloned()
            .ok_or_else(|| AsterError::record_not_found(format!("folder #{folder_id} path")))?;

        for tree_folder_id in &tree_folder_ids {
            let folder_path = folder_paths.get(tree_folder_id).ok_or_else(|| {
                AsterError::record_not_found(format!("folder #{tree_folder_id} path"))
            })?;
            let entry_path = archive_directory_entry_path(&archive_root, folder_path, &root_path)?;
            entries.push(ArchiveEntry::Directory { entry_path });
        }

        for file in tree_files {
            let parent_path = file
                .folder_id
                .and_then(|id| folder_paths.get(&id))
                .ok_or_else(|| {
                    AsterError::record_not_found(format!(
                        "missing parent path for file #{}",
                        file.id
                    ))
                })?;
            let relative_dir = if parent_path == &root_path {
                String::new()
            } else {
                parent_path
                    .strip_prefix(&(root_path.clone() + "/"))
                    .ok_or_else(|| {
                        AsterError::internal_error(format!(
                            "folder path '{parent_path}' is outside root '{root_path}'"
                        ))
                    })?
                    .to_string()
            };
            let entry_path = if relative_dir.is_empty() {
                format!("{archive_root}/{}", file.name)
            } else {
                format!("{archive_root}/{relative_dir}/{}", file.name)
            };
            total_bytes = total_bytes
                .checked_add(file.size)
                .ok_or_else(|| AsterError::internal_error("archive size overflow"))?;
            entries.push(ArchiveEntry::File { file, entry_path });
        }
    }

    entries.sort_by(|left, right| {
        left.entry_path()
            .cmp(right.entry_path())
            .then_with(|| left.is_file().cmp(&right.is_file()))
    });
    Ok((entries, total_bytes))
}

pub(super) async fn resolve_archive_compress_target_folder_id(
    state: &AppState,
    scope: WorkspaceStorageScope,
    selection: &batch_service::NormalizedSelection,
    requested_target_folder_id: Option<i64>,
) -> Result<Option<i64>> {
    if let Some(target_folder_id) = requested_target_folder_id {
        workspace_storage_service::verify_folder_access(state, scope, target_folder_id).await?;
        return Ok(Some(target_folder_id));
    }

    let mut parents = HashSet::new();
    for file_id in &selection.file_ids {
        let file = selection
            .file_map
            .get(file_id)
            .ok_or_else(|| AsterError::file_not_found(format!("file #{file_id}")))?;
        parents.insert(file.folder_id);
    }
    for folder_id in &selection.folder_ids {
        let folder = selection
            .folder_map
            .get(folder_id)
            .ok_or_else(|| AsterError::folder_not_found(format!("folder #{folder_id}")))?;
        parents.insert(folder.parent_id);
    }

    if parents.len() == 1 {
        Ok(parents.into_iter().next().unwrap_or(None))
    } else {
        Ok(None)
    }
}

fn archive_directory_entry_path(
    archive_root: &str,
    folder_path: &str,
    root_path: &str,
) -> Result<String> {
    if folder_path == root_path {
        return Ok(format!("{archive_root}/"));
    }

    let relative_dir = folder_path
        .strip_prefix(&(root_path.to_string() + "/"))
        .ok_or_else(|| {
            AsterError::internal_error(format!(
                "folder path '{folder_path}' is outside root '{root_path}'"
            ))
        })?;
    Ok(format!("{archive_root}/{relative_dir}/"))
}

fn resolve_archive_name(
    archive_name: &Option<String>,
    selection: &batch_service::NormalizedSelection,
) -> Result<String> {
    let base = match archive_name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => default_archive_name(selection),
    };
    let final_name = if base.to_ascii_lowercase().ends_with(".zip") {
        base
    } else {
        format!("{base}.zip")
    };
    crate::utils::validate_name(&final_name)?;
    Ok(final_name)
}

fn default_archive_name(selection: &batch_service::NormalizedSelection) -> String {
    if selection.folder_ids.len() == 1
        && selection.file_ids.is_empty()
        && let Some(folder) = selection.folder_map.get(&selection.folder_ids[0])
    {
        return folder.name.clone();
    }

    if selection.file_ids.len() == 1
        && selection.folder_ids.is_empty()
        && let Some(file) = selection.file_map.get(&selection.file_ids[0])
    {
        return file.name.clone();
    }

    format!("archive-{}", Utc::now().format("%Y%m%d-%H%M%S"))
}
