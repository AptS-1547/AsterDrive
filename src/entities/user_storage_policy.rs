use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, ToSchema)]
#[schema(as = UserStoragePolicy)]
#[sea_orm(table_name = "user_storage_policies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub policy_id: i64,
    pub is_default: bool,
    pub quota_bytes: i64, // 0 = unlimited
    #[schema(value_type = String)]
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::storage_policy::Entity",
        from = "Column::PolicyId",
        to = "super::storage_policy::Column::Id"
    )]
    StoragePolicy,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoragePolicy.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
