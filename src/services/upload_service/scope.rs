use crate::db::repository::upload_session_repo;
use crate::entities::upload_session;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;
use crate::services::workspace_storage_service::{self, WorkspaceStorageScope};

pub(super) fn personal_scope(user_id: i64) -> WorkspaceStorageScope {
    WorkspaceStorageScope::Personal { user_id }
}

pub(super) fn team_scope(team_id: i64, actor_user_id: i64) -> WorkspaceStorageScope {
    WorkspaceStorageScope::Team {
        team_id,
        actor_user_id,
    }
}

fn ensure_personal_upload_session_scope(session: &upload_session::Model) -> Result<()> {
    if session.team_id.is_some() {
        return Err(AsterError::auth_forbidden(
            "upload session belongs to a team workspace",
        ));
    }
    Ok(())
}

fn ensure_team_upload_session_scope(session: &upload_session::Model, team_id: i64) -> Result<()> {
    if session.team_id != Some(team_id) {
        return Err(AsterError::auth_forbidden(
            "upload session is outside team workspace",
        ));
    }
    Ok(())
}

pub(super) async fn load_upload_session(
    state: &AppState,
    scope: WorkspaceStorageScope,
    upload_id: &str,
) -> Result<upload_session::Model> {
    let session = upload_session_repo::find_by_id(&state.db, upload_id).await?;
    crate::utils::verify_owner(session.user_id, scope.actor_user_id(), "upload session")?;
    if let Some(team_id) = scope.team_id() {
        workspace_storage_service::require_team_access(state, team_id, scope.actor_user_id())
            .await?;
        ensure_team_upload_session_scope(&session, team_id)?;
    } else {
        ensure_personal_upload_session_scope(&session)?;
    }
    Ok(session)
}
