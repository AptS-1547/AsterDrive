mod chunk;
mod complete;
mod init;
mod lifecycle;
mod progress;
mod responses;
mod scope;
mod shared;

use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::audit_service::{self, AuditContext};
use crate::services::workspace_models::FileInfo;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};

pub use chunk::{upload_chunk, upload_chunk_for_team};
pub use complete::{complete_upload, complete_upload_for_team};
pub use init::{init_upload, init_upload_for_team};
pub use lifecycle::{cancel_upload, cancel_upload_for_team, cleanup_expired};
pub use progress::{get_progress, get_progress_for_team, presign_parts, presign_parts_for_team};
pub use responses::{ChunkUploadResponse, InitUploadResponse, UploadProgressResponse};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn upload_in_scope_with_audit(
    state: &AppState,
    scope: WorkspaceStorageScope,
    folder_id: Option<i64>,
    relative_path: Option<&str>,
    declared_size: Option<i64>,
    payload: &mut actix_multipart::Multipart,
    audit_ctx: &AuditContext,
) -> Result<FileInfo> {
    let file = workspace_storage_service::upload(
        state,
        scope,
        payload,
        folder_id,
        relative_path,
        declared_size,
    )
    .await?;
    audit_service::log(
        state,
        audit_ctx,
        audit_service::AuditAction::FileUpload,
        Some("file"),
        Some(file.id),
        Some(&file.name),
        None,
    )
    .await;
    Ok(file.into())
}
