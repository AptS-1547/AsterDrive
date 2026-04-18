//! 归档任务子模块：`common`。

use std::io::{Read, Write};

use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::task_service::TaskLeaseGuard;
use crate::services::{folder_service, workspace_storage_service::WorkspaceStorageScope};
use crate::storage::{DriverRegistry, PolicySnapshot};

#[derive(Debug, Clone)]
pub(super) enum ArchiveEntry {
    Directory {
        entry_path: String,
    },
    File {
        file: file::Model,
        entry_path: String,
    },
}

impl ArchiveEntry {
    pub(super) fn entry_path(&self) -> &str {
        match self {
            Self::Directory { entry_path } | Self::File { entry_path, .. } => entry_path,
        }
    }

    pub(super) fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }
}

pub(super) async fn build_folder_display_path(
    db: &DatabaseConnection,
    folder_id: i64,
) -> Result<String> {
    let mut paths = folder_service::build_folder_paths(db, &[folder_id]).await?;
    paths
        .remove(&folder_id)
        .ok_or_else(|| AsterError::record_not_found(format!("folder #{folder_id} path")))
}

pub(super) async fn build_file_display_path(
    db: &DatabaseConnection,
    folder_id: Option<i64>,
    file_name: &str,
) -> Result<String> {
    match folder_id {
        Some(folder_id) => Ok(format!(
            "{}/{}",
            build_folder_display_path(db, folder_id).await?,
            file_name
        )),
        None => Ok(format!("/{file_name}")),
    }
}

pub(super) async fn create_unique_folder_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
    base_name: &str,
) -> Result<folder::Model> {
    let final_name =
        resolve_unique_folder_name_in_scope(state, scope, parent_id, base_name).await?;
    create_folder_exact_in_scope(state, scope, parent_id, &final_name).await
}

pub(super) async fn create_folder_exact_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
    name: &str,
) -> Result<folder::Model> {
    crate::utils::validate_name(name)?;
    let exists = match scope {
        WorkspaceStorageScope::Personal { user_id } => {
            folder_repo::find_by_name_in_parent(&state.db, user_id, parent_id, name)
                .await?
                .is_some()
        }
        WorkspaceStorageScope::Team { team_id, .. } => {
            folder_repo::find_by_name_in_team_parent(&state.db, team_id, parent_id, name)
                .await?
                .is_some()
        }
    };
    if exists {
        return Err(folder_repo::duplicate_name_error(name));
    }

    let now = Utc::now();
    folder_repo::create(
        &state.db,
        folder::ActiveModel {
            name: Set(name.to_string()),
            parent_id: Set(parent_id),
            team_id: Set(scope.team_id()),
            user_id: Set(scope.actor_user_id()),
            policy_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

async fn resolve_unique_folder_name_in_scope(
    state: &AppState,
    scope: WorkspaceStorageScope,
    parent_id: Option<i64>,
    base_name: &str,
) -> Result<String> {
    let mut candidate = base_name.to_string();
    loop {
        let exists = match scope {
            WorkspaceStorageScope::Personal { user_id } => {
                folder_repo::find_by_name_in_parent(&state.db, user_id, parent_id, &candidate)
                    .await?
            }
            WorkspaceStorageScope::Team { team_id, .. } => {
                folder_repo::find_by_name_in_team_parent(&state.db, team_id, parent_id, &candidate)
                    .await?
            }
        };
        if exists.is_none() {
            return Ok(candidate);
        }
        candidate = crate::utils::next_copy_name(&candidate);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_archive_to_sink<W, F>(
    handle: &tokio::runtime::Handle,
    db: &DatabaseConnection,
    driver_registry: &DriverRegistry,
    policy_snapshot: &PolicySnapshot,
    lease_guard: Option<&TaskLeaseGuard>,
    entries: Vec<ArchiveEntry>,
    total_bytes: i64,
    output: W,
    mut on_progress: F,
) -> Result<(W, i64)>
where
    W: Write,
    F: FnMut(i64, &str) -> Result<()>,
{
    let mut zip = zip::ZipWriter::new_stream(output);
    let file_options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let dir_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut processed_bytes = 0_i64;

    for entry in entries {
        ensure_task_lease_active(lease_guard)?;
        match entry {
            ArchiveEntry::Directory { entry_path } => {
                zip.add_directory(&entry_path, dir_options)
                    .map_aster_err(AsterError::storage_driver_error)?;
            }
            ArchiveEntry::File { file, entry_path } => {
                zip.start_file(&entry_path, file_options)
                    .map_aster_err(AsterError::storage_driver_error)?;

                let stream = handle.block_on(async {
                    let blob = file_repo::find_blob_by_id(db, file.blob_id).await?;
                    let policy = policy_snapshot.get_policy_or_err(blob.policy_id)?;
                    let driver = driver_registry.get_driver(&policy)?;
                    driver.get_stream(&blob.storage_path).await
                })?;

                let mut reader = tokio_util::io::SyncIoBridge::new(stream);
                let copied = copy_reader_to_writer_with_lease(lease_guard, &mut reader, &mut zip)?;
                processed_bytes = processed_bytes
                    .checked_add(i64::try_from(copied).map_err(|_| {
                        AsterError::internal_error(format!(
                            "copied bytes exceed i64 range: {copied}"
                        ))
                    })?)
                    .ok_or_else(|| AsterError::internal_error("archive progress overflow"))?;

                on_progress(processed_bytes, &entry_path)?;
            }
        }
    }

    let output = zip
        .finish()
        .map_aster_err(AsterError::storage_driver_error)?
        .into_inner();
    Ok((output, processed_bytes.max(total_bytes)))
}

pub(super) fn is_client_disconnect_error_text(error_text: &str) -> bool {
    error_text.contains("Broken pipe")
        || error_text.contains("Connection reset by peer")
        || error_text.contains("connection closed")
}

pub(super) fn copy_reader_to_writer_with_lease<R: Read, W: Write>(
    lease_guard: Option<&TaskLeaseGuard>,
    reader: &mut R,
    writer: &mut W,
) -> Result<u64> {
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        ensure_task_lease_active(lease_guard)?;
        let read = reader.read(&mut buffer).map_aster_err_ctx(
            "read archive stream chunk",
            AsterError::storage_driver_error,
        )?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_aster_err_ctx(
            "write archive stream chunk",
            AsterError::storage_driver_error,
        )?;
        copied = copied
            .checked_add(u64::try_from(read).map_err(|_| {
                AsterError::internal_error("archive stream chunk size exceeds u64 range")
            })?)
            .ok_or_else(|| AsterError::internal_error("archive stream byte counter overflow"))?;
    }

    Ok(copied)
}

fn ensure_task_lease_active(lease_guard: Option<&TaskLeaseGuard>) -> Result<()> {
    if let Some(lease_guard) = lease_guard {
        lease_guard.ensure_active()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::thread;
    use std::time::Duration;

    use crate::services::task_service::{
        TaskLease, TaskLeaseGuard, is_task_lease_renewal_timed_out,
    };

    use super::copy_reader_to_writer_with_lease;

    struct SlowSingleChunkReader {
        chunk: Vec<u8>,
        delay: Duration,
        consumed: bool,
    }

    impl Read for SlowSingleChunkReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.consumed {
                return Ok(0);
            }
            thread::sleep(self.delay);
            let len = self.chunk.len().min(buf.len());
            buf[..len].copy_from_slice(&self.chunk[..len]);
            self.consumed = true;
            Ok(len)
        }
    }

    #[test]
    fn copy_reader_to_writer_with_lease_stops_after_renewal_timeout() {
        let lease_guard =
            TaskLeaseGuard::with_renewal_timeout(TaskLease::new(42, 7), Duration::from_millis(20));
        let mut reader = SlowSingleChunkReader {
            chunk: b"chunk".to_vec(),
            delay: Duration::from_millis(30),
            consumed: false,
        };
        let mut writer = Vec::new();

        let error = copy_reader_to_writer_with_lease(Some(&lease_guard), &mut reader, &mut writer)
            .expect_err("expired lease should stop blocking copy loop");

        assert!(is_task_lease_renewal_timed_out(&error));
        assert_eq!(writer, b"chunk");
    }
}
