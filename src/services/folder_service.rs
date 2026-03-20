use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};
use serde::Serialize;

use crate::db::repository::{file_repo, folder_repo};
use crate::entities::{file, folder};
use crate::errors::{AsterError, Result};

#[derive(Serialize)]
pub struct FolderContents {
    pub folders: Vec<folder::Model>,
    pub files: Vec<file::Model>,
}

pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    name: &str,
    parent_id: Option<i64>,
) -> Result<folder::Model> {
    // 检查同名文件夹
    if folder_repo::find_by_name_in_parent(db, user_id, parent_id, name)
        .await?
        .is_some()
    {
        return Err(AsterError::validation_error(format!(
            "folder '{}' already exists in this location",
            name
        )));
    }

    let now = Utc::now();
    let model = folder::ActiveModel {
        name: Set(name.to_string()),
        parent_id: Set(parent_id),
        user_id: Set(user_id),
        policy_id: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    folder_repo::create(db, model).await
}

pub async fn list(
    db: &DatabaseConnection,
    user_id: i64,
    parent_id: Option<i64>,
) -> Result<FolderContents> {
    let folders = folder_repo::find_children(db, user_id, parent_id).await?;
    let files = file_repo::find_by_folder(db, user_id, parent_id).await?;
    Ok(FolderContents { folders, files })
}

pub async fn delete(db: &DatabaseConnection, id: i64, user_id: i64) -> Result<()> {
    let folder = folder_repo::find_by_id(db, id).await?;
    if folder.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your folder"));
    }
    folder_repo::delete(db, id).await
}

pub async fn update(
    db: &DatabaseConnection,
    id: i64,
    user_id: i64,
    name: Option<String>,
    parent_id: Option<i64>,
    policy_id: Option<i64>,
) -> Result<folder::Model> {
    let f = folder_repo::find_by_id(db, id).await?;
    if f.user_id != user_id {
        return Err(AsterError::auth_forbidden("not your folder"));
    }
    let mut active: folder::ActiveModel = f.into();
    if let Some(n) = name {
        active.name = Set(n);
    }
    if let Some(pid) = parent_id {
        active.parent_id = Set(Some(pid));
    }
    if let Some(pid) = policy_id {
        active.policy_id = Set(Some(pid));
    }
    active.updated_at = Set(Utc::now());
    use sea_orm::ActiveModelTrait;
    active.update(db).await.map_err(AsterError::from)
}
