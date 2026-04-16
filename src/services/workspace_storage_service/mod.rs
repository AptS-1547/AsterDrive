mod blob_upload;
mod multipart;
mod store;
#[cfg(test)]
mod tests;

pub(crate) use crate::services::workspace_scope_service::{
    WorkspaceStorageScope, ensure_active_file_scope, ensure_active_folder_scope, ensure_file_scope,
    ensure_folder_scope, ensure_personal_file_scope, list_files_in_folder, list_folders_in_parent,
    require_scope_access, require_team_access, require_team_management_access, verify_file_access,
    verify_folder_access,
};
pub(crate) use crate::services::workspace_storage_core::{
    check_quota, create_exact_file_from_blob, create_new_file_from_blob, create_nondedup_blob,
    create_nondedup_blob_with_key, create_s3_nondedup_blob, ensure_upload_parent_path,
    finalize_upload_session_blob, finalize_upload_session_file, load_storage_limits,
    local_content_dedup_enabled, parse_relative_upload_path, resolve_policy_for_size,
    update_storage_used,
};

pub(crate) use blob_upload::{
    cleanup_preuploaded_blob_upload, persist_preuploaded_blob, prepare_non_dedup_blob_upload,
    upload_temp_file_to_prepared_blob,
};
pub(crate) use multipart::upload;
pub(crate) use store::{
    create_empty, store_from_temp, store_from_temp_exact_name_with_hints,
    store_from_temp_with_hints,
};

const HASH_BUF_SIZE: usize = 65536;

#[derive(Clone, Copy)]
enum NewFileMode {
    ResolveUnique,
    Exact,
}
