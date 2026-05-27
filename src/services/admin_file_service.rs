//! Admin observability service for files and file blobs.

use std::collections::HashSet;

use crate::api::dto::admin::{
    AdminFileBlobDetail, AdminFileBlobHashKind, AdminFileBlobInfo, AdminFileBlobListQuery,
    AdminFileBlobReferenceFile, AdminFileBlobReferenceVersion, AdminFileBlobSummary,
    AdminFileDetail, AdminFileInfo, AdminFileListQuery, AdminFileVersionSummary,
};
use crate::api::pagination::{OffsetPage, load_offset_page};
use crate::db::repository::{file_repo, version_repo};
use crate::entities::{file, file_blob, file_version};
use crate::errors::{AsterError, Result};
use crate::runtime::PrimaryAppState;

pub async fn list_files(
    state: &PrimaryAppState,
    limit: u64,
    offset: u64,
    query: &AdminFileListQuery,
) -> Result<OffsetPage<AdminFileInfo>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) = file_repo::find_admin_files_paginated(
            state.reader_db(),
            limit,
            offset,
            file_repo::AdminFileFilters {
                name: query.name.as_deref(),
                blob_id: query.blob_id,
                policy_id: query.policy_id,
                owner_user_id: query.owner_user_id,
                team_id: query.team_id,
                deleted: query.deleted,
                sort_by: query.sort_by(),
                sort_order: query.sort_order(),
            },
        )
        .await?;
        Ok((items.into_iter().map(to_admin_file_info).collect(), total))
    })
    .await
}

pub async fn get_file(state: &PrimaryAppState, file_id: i64) -> Result<AdminFileDetail> {
    let (file, blob) = file_repo::find_admin_file_by_id(state.reader_db(), file_id).await?;
    let versions = version_repo::find_by_file_id(state.reader_db(), file_id).await?;
    let version_blob_ids = versions
        .iter()
        .map(|version| version.blob_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let version_blobs = file_repo::find_blobs_by_ids(state.reader_db(), &version_blob_ids).await?;
    let versions = versions
        .into_iter()
        .map(|version| {
            let blob = version_blobs
                .get(&version.blob_id)
                .cloned()
                .ok_or_else(|| {
                    AsterError::internal_error(format!(
                        "file_version #{} references missing blob #{}",
                        version.id, version.blob_id
                    ))
                })?;
            Ok(to_admin_version_summary(version, blob))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(AdminFileDetail {
        file: to_admin_file_info((file, blob)),
        versions,
    })
}

pub async fn list_blobs(
    state: &PrimaryAppState,
    limit: u64,
    offset: u64,
    query: &AdminFileBlobListQuery,
) -> Result<OffsetPage<AdminFileBlobInfo>> {
    load_offset_page(limit, offset, 100, |limit, offset| async move {
        let (items, total) = file_repo::find_admin_blobs_paginated(
            state.reader_db(),
            limit,
            offset,
            file_repo::AdminFileBlobFilters {
                hash: query.hash.as_deref(),
                policy_id: query.policy_id,
                storage_path: query.storage_path.as_deref(),
                ref_count_min: query.ref_count_min,
                ref_count_max: query.ref_count_max,
                size_min: query.size_min,
                size_max: query.size_max,
                sort_by: query.sort_by(),
                sort_order: query.sort_order(),
            },
        )
        .await?;
        Ok((items.into_iter().map(to_admin_blob_info).collect(), total))
    })
    .await
}

pub async fn get_blob(state: &PrimaryAppState, blob_id: i64) -> Result<AdminFileBlobDetail> {
    let blob = file_repo::find_blob_by_id(state.reader_db(), blob_id).await?;
    let files = file_repo::find_by_blob_id(state.reader_db(), blob_id).await?;
    let versions = version_repo::find_by_blob_id(state.reader_db(), blob_id).await?;

    Ok(AdminFileBlobDetail {
        blob: to_admin_blob_info(blob),
        files: files.into_iter().map(to_blob_reference_file).collect(),
        file_versions: versions
            .into_iter()
            .map(to_blob_reference_version)
            .collect(),
    })
}

fn to_admin_file_info((file, blob): (file::Model, file_blob::Model)) -> AdminFileInfo {
    AdminFileInfo {
        id: file.id,
        name: file.name,
        folder_id: file.folder_id,
        team_id: file.team_id,
        blob_id: file.blob_id,
        size: file.size,
        owner_user_id: file.owner_user_id,
        created_by_user_id: file.created_by_user_id,
        created_by_username: file.created_by_username,
        mime_type: file.mime_type,
        extension: file.extension,
        compound_extension: file.compound_extension,
        file_category: file.file_category,
        created_at: file.created_at,
        updated_at: file.updated_at,
        deleted_at: file.deleted_at,
        is_locked: file.is_locked,
        blob: to_blob_summary(blob),
    }
}

fn to_admin_version_summary(
    version: file_version::Model,
    blob: file_blob::Model,
) -> AdminFileVersionSummary {
    AdminFileVersionSummary {
        id: version.id,
        file_id: version.file_id,
        blob_id: version.blob_id,
        version: version.version,
        size: version.size,
        created_at: version.created_at,
        blob: to_blob_summary(blob),
    }
}

fn to_blob_summary(blob: file_blob::Model) -> AdminFileBlobSummary {
    AdminFileBlobSummary {
        id: blob.id,
        hash: blob.hash,
        size: blob.size,
        policy_id: blob.policy_id,
        storage_path: blob.storage_path,
    }
}

fn to_admin_blob_info(blob: file_blob::Model) -> AdminFileBlobInfo {
    let hash_kind = blob_hash_kind(&blob.hash);
    AdminFileBlobInfo {
        id: blob.id,
        hash: blob.hash,
        size: blob.size,
        policy_id: blob.policy_id,
        storage_path: blob.storage_path,
        thumbnail_path: blob.thumbnail_path,
        thumbnail_processor: blob.thumbnail_processor,
        thumbnail_version: blob.thumbnail_version,
        ref_count: blob.ref_count,
        created_at: blob.created_at,
        updated_at: blob.updated_at,
        hash_kind,
    }
}

fn to_blob_reference_file(file: file::Model) -> AdminFileBlobReferenceFile {
    AdminFileBlobReferenceFile {
        id: file.id,
        name: file.name,
        folder_id: file.folder_id,
        team_id: file.team_id,
        owner_user_id: file.owner_user_id,
        size: file.size,
        mime_type: file.mime_type,
        created_at: file.created_at,
        updated_at: file.updated_at,
        deleted_at: file.deleted_at,
    }
}

fn to_blob_reference_version(version: file_version::Model) -> AdminFileBlobReferenceVersion {
    AdminFileBlobReferenceVersion {
        id: version.id,
        file_id: version.file_id,
        version: version.version,
        size: version.size,
        created_at: version.created_at,
    }
}

fn blob_hash_kind(hash: &str) -> AdminFileBlobHashKind {
    if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        AdminFileBlobHashKind::ContentSha256
    } else {
        AdminFileBlobHashKind::Opaque
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminFileBlobHashKind, blob_hash_kind};

    #[test]
    fn blob_hash_kind_detects_content_sha256() {
        assert_eq!(
            blob_hash_kind("0123456789abcdef0123456789abcdef0123456789ABCDEF0123456789ABCDEF"),
            AdminFileBlobHashKind::ContentSha256
        );
        assert_eq!(blob_hash_kind("not-sha256"), AdminFileBlobHashKind::Opaque);
    }
}
