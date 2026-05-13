use crate::entities::storage_policy;
use crate::errors::{AsterError, file_upload_error_with_subcode, validation_error_with_subcode};
use crate::services::workspace_storage_service::WorkspaceStorageScope;

#[derive(Clone, Copy)]
pub(super) struct DirectUploadParams<'a> {
    pub scope: WorkspaceStorageScope,
    pub folder_id: Option<i64>,
    pub relative_path: Option<&'a str>,
    pub resolved_filename: &'a str,
    pub policy: &'a storage_policy::Model,
    pub declared_size: i64,
    pub actor_username: Option<&'a str>,
}

pub(super) fn upload_field_read_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.field_read_failed", message)
}

pub(super) fn upload_local_staging_path_resolve_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_path_resolve_failed", message)
}

pub(super) fn upload_local_staging_dir_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_dir_create_failed", message)
}

pub(super) fn upload_local_staging_file_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_file_create_failed", message)
}

pub(super) fn upload_local_staging_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_write_failed", message)
}

pub(super) fn upload_local_staging_flush_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.local_staging_flush_failed", message)
}

pub(super) fn upload_direct_relay_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.direct_relay_write_failed", message)
}

pub(super) fn upload_direct_relay_shutdown_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.direct_relay_shutdown_failed", message)
}

pub(super) fn upload_temp_dir_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_dir_create_failed", message)
}

pub(super) fn upload_temp_file_create_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_create_failed", message)
}

pub(super) fn upload_temp_file_write_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_write_failed", message)
}

pub(super) fn upload_temp_file_flush_failed(message: String) -> AsterError {
    file_upload_error_with_subcode("upload.temp_file_flush_failed", message)
}

pub(super) fn upload_body_size_overflow_error() -> AsterError {
    file_upload_error_with_subcode(
        "upload.body_size_overflow",
        "accumulated chunk size overflows i64",
    )
}

pub(super) fn upload_empty_file_error() -> AsterError {
    validation_error_with_subcode("upload.empty_file", "empty file")
}

pub(super) fn upload_size_mismatch_error(declared_size: i64, actual_size: i64) -> AsterError {
    AsterError::validation_error(format!(
        "size mismatch: declared {} bytes, received {} bytes",
        declared_size, actual_size
    ))
}
