//! 归档解包任务子模块：`staging`。

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::db::repository::file_repo;
use crate::entities::file;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::task_service::TaskStepInfo;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};
use crate::storage::PolicySnapshot;

use super::super::super::TaskLeaseGuard;
use super::super::super::steps::{
    TASK_STEP_EXTRACT_ARCHIVE, set_task_step_active, set_task_step_succeeded,
};
use super::super::common::copy_reader_to_writer_with_lease_and_expected_size;

#[derive(Debug)]
pub(super) struct StagedArchiveStats {
    pub(super) total_bytes: i64,
    pub(super) total_progress: i64,
    pub(super) file_count: i64,
    pub(super) directory_count: i64,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ArchiveExtractPolicyResolver {
    Personal { user_id: i64 },
    Team { policy_group_id: i64 },
}

impl ArchiveExtractPolicyResolver {
    fn ensure_entry_size_allowed(
        self,
        policy_snapshot: &PolicySnapshot,
        entry_size: i64,
    ) -> Result<()> {
        let policy = match self {
            Self::Personal { user_id } => {
                policy_snapshot.resolve_user_policy_for_size(user_id, entry_size)?
            }
            Self::Team { policy_group_id } => {
                policy_snapshot.resolve_policy_in_group(policy_group_id, entry_size)?
            }
        };
        if policy.max_file_size > 0 && entry_size > policy.max_file_size {
            return Err(AsterError::file_too_large(format!(
                "archive entry size {} exceeds limit {}",
                entry_size, policy.max_file_size
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArchiveExtractStageOptions {
    pub(super) scope: WorkspaceStorageScope,
    pub(super) policy_resolver: ArchiveExtractPolicyResolver,
    pub(super) source_archive_size: i64,
    pub(super) max_staging_bytes: i64,
}

#[derive(Clone, Copy)]
pub(super) struct StageZipArchiveForExtractParams<'a> {
    pub(super) handle: &'a tokio::runtime::Handle,
    pub(super) db: &'a sea_orm::DatabaseConnection,
    pub(super) policy_snapshot: &'a PolicySnapshot,
    pub(super) lease_guard: &'a TaskLeaseGuard,
    pub(super) archive_path: &'a str,
    pub(super) stage_root: &'a str,
    pub(super) options: ArchiveExtractStageOptions,
}

pub(super) async fn download_file_to_temp(
    state: &AppState,
    source_file: &file::Model,
    temp_path: &Path,
) -> Result<()> {
    let blob = file_repo::find_blob_by_id(&state.db, source_file.blob_id).await?;
    let policy = state.policy_snapshot.get_policy_or_err(blob.policy_id)?;
    let driver = state.driver_registry.get_driver(&policy)?;
    let mut stream = driver.get_stream(&blob.storage_path).await?;
    let mut output = tokio::fs::File::create(temp_path).await.map_aster_err_ctx(
        "create source archive temp file",
        AsterError::storage_driver_error,
    )?;
    tokio::io::copy(&mut stream, &mut output)
        .await
        .map_aster_err_ctx("download source archive", AsterError::storage_driver_error)?;
    output.flush().await.map_aster_err_ctx(
        "flush source archive temp file",
        AsterError::storage_driver_error,
    )?;
    Ok(())
}

pub(super) fn stage_zip_archive_for_extract(
    params: StageZipArchiveForExtractParams<'_>,
    steps: &mut [TaskStepInfo],
) -> Result<StagedArchiveStats> {
    let StageZipArchiveForExtractParams {
        handle,
        db,
        policy_snapshot,
        lease_guard,
        archive_path,
        stage_root,
        options,
    } = params;
    let file = std::fs::File::open(archive_path)
        .map_aster_err_ctx("open source archive", AsterError::storage_driver_error)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_aster_err_with(|| AsterError::validation_error("invalid zip archive"))?;
    set_task_step_active(
        steps,
        TASK_STEP_EXTRACT_ARCHIVE,
        Some("Reading archive"),
        None,
    )?;
    handle.block_on(async {
        super::super::super::update_task_progress_db(
            db,
            lease_guard,
            0,
            0,
            Some("Reading archive"),
            steps,
        )
        .await
    })?;
    let mut total_bytes = 0_i64;
    for index in 0..archive.len() {
        lease_guard.ensure_active()?;
        let entry = archive
            .by_index(index)
            .map_aster_err_with(|| AsterError::validation_error("invalid zip archive entry"))?;
        if entry.is_dir() {
            continue;
        }
        let entry_size = crate::utils::numbers::u64_to_i64(entry.size(), "archive entry size")?;
        options
            .policy_resolver
            .ensure_entry_size_allowed(policy_snapshot, entry_size)?;
        total_bytes = total_bytes
            .checked_add(entry_size)
            .ok_or_else(|| AsterError::internal_error("archive extract size overflow"))?;
    }
    let total_staging_bytes = options
        .source_archive_size
        .checked_add(total_bytes)
        .ok_or_else(|| AsterError::internal_error("archive extract staging size overflow"))?;
    if total_staging_bytes > options.max_staging_bytes {
        return Err(AsterError::validation_error(format!(
            "archive extract staging requires {} bytes (source {} + extracted {}), exceeds server limit {}",
            total_staging_bytes,
            options.source_archive_size,
            total_bytes,
            options.max_staging_bytes
        )));
    }
    handle.block_on(async {
        workspace_storage_service::check_quota(db, options.scope, total_bytes).await
    })?;
    let total_progress = total_bytes
        .checked_mul(2)
        .ok_or_else(|| AsterError::internal_error("archive extract progress overflow"))?;
    set_task_step_active(
        steps,
        TASK_STEP_EXTRACT_ARCHIVE,
        Some("Reading archive"),
        Some((0, total_bytes)),
    )?;
    handle.block_on(async {
        super::super::super::update_task_progress_db(
            db,
            lease_guard,
            0,
            total_progress,
            Some("Reading archive"),
            steps,
        )
        .await
    })?;

    let mut processed_bytes = 0_i64;
    let mut created_dirs = HashSet::new();
    let mut file_count = 0_i64;

    for index in 0..archive.len() {
        lease_guard.ensure_active()?;
        let mut entry = archive
            .by_index(index)
            .map_aster_err_with(|| AsterError::validation_error("invalid zip archive entry"))?;
        let declared_size = crate::utils::numbers::u64_to_i64(entry.size(), "archive entry size")?;
        let enclosed_path = entry.enclosed_name().ok_or_else(|| {
            AsterError::validation_error(format!(
                "archive entry '{}' contains unsafe path",
                entry.name()
            ))
        })?;
        let relative_path = normalize_archive_entry_path(&enclosed_path)?;
        let target_path = Path::new(stage_root).join(&relative_path);
        if entry.is_dir() {
            register_relative_dirs(&mut created_dirs, &relative_path);
            std::fs::create_dir_all(&target_path).map_aster_err_ctx(
                "create extracted directory",
                AsterError::storage_driver_error,
            )?;
            continue;
        }

        if let Some(parent) = relative_path.parent() {
            register_relative_dirs(&mut created_dirs, parent);
        }
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_aster_err_ctx(
                "create extracted parent directory",
                AsterError::storage_driver_error,
            )?;
        }

        let mut output = std::fs::File::create(&target_path)
            .map_aster_err_ctx("create extracted file", AsterError::storage_driver_error)?;
        let entry_context = format!("archive entry '{}'", relative_path.display());
        let copied = copy_reader_to_writer_with_lease_and_expected_size(
            Some(lease_guard),
            &mut entry,
            &mut output,
            crate::utils::numbers::i64_to_u64(declared_size, "archive entry size")?,
            &entry_context,
        )?;
        processed_bytes = processed_bytes
            .checked_add(crate::utils::numbers::u64_to_i64(
                copied,
                "extracted bytes",
            )?)
            .ok_or_else(|| AsterError::internal_error("archive extract progress overflow"))?;
        file_count += 1;

        let status_text = format!("Extracting {}", relative_path.to_string_lossy());
        set_task_step_active(
            steps,
            TASK_STEP_EXTRACT_ARCHIVE,
            Some(&status_text),
            Some((processed_bytes, total_bytes)),
        )?;
        handle.block_on(async {
            super::super::super::update_task_progress_db(
                db,
                lease_guard,
                processed_bytes,
                total_progress,
                Some(&status_text),
                steps,
            )
            .await
        })?;
    }

    set_task_step_succeeded(
        steps,
        TASK_STEP_EXTRACT_ARCHIVE,
        Some("Archive extracted to staging"),
        Some((total_bytes, total_bytes)),
    )?;

    Ok(StagedArchiveStats {
        total_bytes,
        total_progress,
        file_count,
        directory_count: i64::try_from(created_dirs.len()).map_aster_err_with(|| {
            AsterError::internal_error("directory count exceeds i64 range")
        })?,
    })
}

fn normalize_archive_entry_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(|| {
                    AsterError::validation_error("archive entry name must be valid UTF-8")
                })?;
                crate::utils::validate_name(name)?;
                normalized.push(name);
            }
            _ => {
                return Err(AsterError::validation_error(format!(
                    "archive entry '{}' contains invalid path component",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AsterError::validation_error(
            "archive entry path cannot be empty",
        ));
    }
    Ok(normalized)
}

fn register_relative_dirs(created_dirs: &mut HashSet<PathBuf>, path: &Path) {
    let mut current = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(name) = component {
            current.push(name);
            created_dirs.insert(current.clone());
        }
    }
}
