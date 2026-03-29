use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, ToSchema)]
#[schema(as = UploadSessionPart)]
#[sea_orm(table_name = "upload_session_parts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub upload_id: String,
    pub part_number: i32,
    pub etag: String,
    pub size: i64,
    #[schema(value_type = String)]
    pub created_at: DateTimeUtc,
    #[schema(value_type = String)]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::upload_session::Entity",
        from = "Column::UploadId",
        to = "super::upload_session::Column::Id",
        on_delete = "Cascade"
    )]
    UploadSession,
}

impl Related<super::upload_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UploadSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
