use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::types::EntityType;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, ToSchema)]
#[schema(as = EntityProperty)]
#[sea_orm(table_name = "entity_properties")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub namespace: String,
    pub name: String,
    pub value: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
