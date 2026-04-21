//! SeaORM 实体定义：`master_binding`。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
#[sea_orm(table_name = "master_bindings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub master_url: String,
    pub access_key: String,
    #[serde(skip_serializing)]
    pub secret_key: String,
    pub namespace: String,
    pub ingress_policy_id: i64,
    pub is_enabled: bool,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: DateTimeUtc,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::storage_policy::Entity",
        from = "Column::IngressPolicyId",
        to = "super::storage_policy::Column::Id"
    )]
    IngressPolicy,
}

impl Related<super::storage_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::IngressPolicy.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
