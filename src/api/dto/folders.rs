use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// Create a new folder.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateFolderReq {
    pub name: String,
    pub parent_id: Option<i64>,
}

/// Patch (partial update) a folder.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct PatchFolderReq {
    pub name: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<i64>)
    )]
    pub parent_id: crate::types::NullablePatch<i64>,
    #[serde(default)]
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<i64>)
    )]
    pub policy_id: crate::types::NullablePatch<i64>,
}

/// Lock or unlock a folder.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct SetLockReq {
    pub locked: bool,
}

/// Copy a folder to a target location.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CopyFolderReq {
    /// Target parent folder ID (`None` = root directory).
    pub parent_id: Option<i64>,
}
