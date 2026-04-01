use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::types::{UserRole, UserStatus};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), schema(as = UserInfo))]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub username: String,
    #[sea_orm(unique)]
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub status: UserStatus,
    #[serde(skip_serializing)]
    pub session_version: i64,
    pub storage_used: i64,
    pub storage_quota: i64, // 0 = unlimited
    pub policy_group_id: Option<i64>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTimeUtc,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
    #[serde(skip_serializing)]
    pub config: Option<String>, // JSON blob, nullable
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::file::Entity")]
    Files,
    #[sea_orm(has_many = "super::folder::Entity")]
    Folders,
    #[sea_orm(has_many = "super::user_storage_policy::Entity")]
    UserStoragePolicies,
    #[sea_orm(
        belongs_to = "super::storage_policy_group::Entity",
        from = "Column::PolicyGroupId",
        to = "super::storage_policy_group::Column::Id"
    )]
    StoragePolicyGroup,
}

impl Related<super::file::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Files.def()
    }
}

impl Related<super::folder::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Folders.def()
    }
}

impl Related<super::user_storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserStoragePolicies.def()
    }
}

impl Related<super::storage_policy_group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoragePolicyGroup.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
