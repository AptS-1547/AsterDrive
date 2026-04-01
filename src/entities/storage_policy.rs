use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

use crate::types::DriverType;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[cfg_attr(all(debug_assertions, feature = "openapi"), schema(as = StoragePolicy))]
#[sea_orm(table_name = "storage_policies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub driver_type: DriverType,
    pub endpoint: String,
    pub bucket: String,
    #[serde(skip_serializing)]
    pub access_key: String,
    #[serde(skip_serializing)]
    pub secret_key: String,
    pub base_path: String,
    pub max_file_size: i64,    // 0 = unlimited
    pub allowed_types: String, // JSON array
    pub options: String,       // JSON object
    pub is_default: bool,
    pub chunk_size: i64, // 0 = single upload, >0 = chunk size in bytes
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTimeUtc,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_storage_policy::Entity")]
    UserStoragePolicies,
    #[sea_orm(has_many = "super::storage_policy_group_item::Entity")]
    StoragePolicyGroupItems,
    #[sea_orm(has_many = "super::file_blob::Entity")]
    FileBlobs,
    #[sea_orm(has_many = "super::folder::Entity")]
    Folders,
}

impl Related<super::user_storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserStoragePolicies.def()
    }
}

impl Related<super::storage_policy_group_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoragePolicyGroupItems.def()
    }
}

impl Related<super::file_blob::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FileBlobs.def()
    }
}

impl Related<super::folder::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Folders.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
