//! SeaORM 实体定义：`managed_follower`。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(table_name = "managed_followers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub access_key: String,
    #[serde(skip_serializing)]
    pub secret_key: String,
    pub namespace: String,
    pub is_enabled: bool,
    pub last_capabilities: String,
    pub last_error: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = Option<String>))]
    pub last_checked_at: Option<DateTimeUtc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTimeUtc,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::follower_enrollment_session::Entity")]
    FollowerEnrollmentSessions,
    #[sea_orm(has_many = "super::storage_policy::Entity")]
    StoragePolicies,
}

impl Related<super::follower_enrollment_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FollowerEnrollmentSessions.def()
    }
}

impl Related<super::storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoragePolicies.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
