use crate::db::repository::{file_repo, folder_repo, property_repo};
use crate::entities::entity_property;
use crate::errors::{AsterError, Result};
use crate::runtime::AppState;

/// 验证实体归属并返回（entity_type 必须是 "file" 或 "folder"）
async fn verify_ownership(
    state: &AppState,
    entity_type: &str,
    entity_id: i64,
    user_id: i64,
) -> Result<()> {
    match entity_type {
        "file" => {
            let f = file_repo::find_by_id(&state.db, entity_id).await?;
            if f.user_id != user_id {
                return Err(AsterError::auth_forbidden("not your file"));
            }
        }
        "folder" => {
            let f = folder_repo::find_by_id(&state.db, entity_id).await?;
            if f.user_id != user_id {
                return Err(AsterError::auth_forbidden("not your folder"));
            }
        }
        _ => {
            return Err(AsterError::validation_error(
                "entity_type must be 'file' or 'folder'",
            ));
        }
    }
    Ok(())
}

/// 列出实体的所有属性
pub async fn list(
    state: &AppState,
    entity_type: &str,
    entity_id: i64,
    user_id: i64,
) -> Result<Vec<entity_property::Model>> {
    verify_ownership(state, entity_type, entity_id, user_id).await?;
    property_repo::find_by_entity(&state.db, entity_type, entity_id).await
}

/// 设置（新增/更新）属性
pub async fn set(
    state: &AppState,
    entity_type: &str,
    entity_id: i64,
    user_id: i64,
    namespace: &str,
    name: &str,
    value: Option<&str>,
) -> Result<entity_property::Model> {
    verify_ownership(state, entity_type, entity_id, user_id).await?;

    if namespace == "DAV:" {
        return Err(AsterError::auth_forbidden("DAV: namespace is read-only"));
    }

    property_repo::upsert(&state.db, entity_type, entity_id, namespace, name, value).await
}

/// 删除单个属性
pub async fn delete(
    state: &AppState,
    entity_type: &str,
    entity_id: i64,
    user_id: i64,
    namespace: &str,
    name: &str,
) -> Result<()> {
    verify_ownership(state, entity_type, entity_id, user_id).await?;

    if namespace == "DAV:" {
        return Err(AsterError::auth_forbidden("DAV: namespace is read-only"));
    }

    property_repo::delete_prop(&state.db, entity_type, entity_id, namespace, name).await
}
