use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

/// Path parameters for entity-scoped property routes.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct EntityPath {
    pub entity_type: crate::types::EntityType,
    pub entity_id: i64,
}

/// Path parameters for individual property routes.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct PropPath {
    pub entity_type: crate::types::EntityType,
    pub entity_id: i64,
    pub namespace: String,
    pub name: String,
}

/// Set or delete a custom property on an entity.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetPropReq {
    pub namespace: String,
    pub name: String,
    pub value: Option<String>,
}
