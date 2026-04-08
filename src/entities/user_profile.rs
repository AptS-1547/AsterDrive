#![allow(deprecated)]

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::types::AvatarSource;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), schema(as = UserProfileModel))]
#[sea_orm(table_name = "user_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i64,
    pub display_name: Option<String>,
    pub avatar_source: AvatarSource,
    #[deprecated(
        since = "0.1.0",
        note = "legacy avatar storage policy compatibility; new avatar uploads always use system_config.avatar_dir local storage"
    )]
    pub avatar_policy_id: Option<i64>,
    pub avatar_key: Option<String>,
    pub avatar_version: i32,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTimeUtc,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
}

#[allow(deprecated)]
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
    #[allow(deprecated)]
    #[deprecated(
        since = "0.1.0",
        note = "legacy avatar storage policy compatibility; new avatar uploads always use system_config.avatar_dir local storage"
    )]
    #[sea_orm(
        belongs_to = "super::storage_policy::Entity",
        from = "Column::AvatarPolicyId",
        to = "super::storage_policy::Column::Id"
    )]
    StoragePolicy,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

#[allow(deprecated)]
impl Related<super::storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoragePolicy.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
