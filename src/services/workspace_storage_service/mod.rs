//! 统一工作空间文件链路的 façade。
//!
//! route 层通常不直接区分“个人上传逻辑”和“团队上传逻辑”，而是先构造
//! `WorkspaceStorageScope`，再从这里进入统一的文件主链路。这个模块本身
//! 主要负责把 scope 校验、核心存储动作和不同上传入口重新导出成一个稳定入口。

mod blob_upload;
mod multipart;
mod store;
#[cfg(test)]
mod tests;

// 调用方只需要依赖 `workspace_storage_service`，不必同时了解 scope helper
// 和底层核心实现分别散落在哪个文件里。
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
    PreparedNonDedupBlobUpload, cleanup_preuploaded_blob_upload, persist_preuploaded_blob,
    prepare_non_dedup_blob_upload, upload_temp_file_to_prepared_blob,
};
pub(crate) use multipart::{relay_stream_direct_upload_eligible, upload};
pub(crate) use store::{
    create_empty, store_from_temp, store_from_temp_exact_name_with_hints,
    store_from_temp_with_hints, store_preuploaded_nondedup,
};

// Local content-dedup 会在不把整文件读入内存的前提下流式计算 SHA-256。
const HASH_BUF_SIZE: usize = 65536;

#[derive(Clone, Copy)]
enum NewFileMode {
    ResolveUnique,
    Exact,
}
