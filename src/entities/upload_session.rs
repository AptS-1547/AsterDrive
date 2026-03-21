use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::types::UploadSessionStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, ToSchema)]
#[schema(as = UploadSession)]
#[sea_orm(table_name = "upload_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String, // UUID
    pub user_id: i64,
    pub filename: String,
    pub total_size: i64,
    pub chunk_size: i64,
    pub total_chunks: i32,
    pub received_count: i32,
    pub folder_id: Option<i64>,
    pub policy_id: i64,
    pub status: UploadSessionStatus,
    pub s3_temp_key: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTimeUtc,
    #[schema(value_type = String)]
    pub expires_at: DateTimeUtc,
    #[schema(value_type = String)]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
