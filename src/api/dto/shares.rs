use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// Create a new share.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateShareReq {
    pub target: crate::services::share_service::ShareTarget,
    pub password: Option<String>,
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<String>)
    )]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub max_downloads: i64,
}

/// Update an existing share.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct UpdateShareReq {
    /// `None` = keep existing password, `Some("")` = remove password,
    /// non-empty = replace password.
    pub password: Option<String>,
    #[cfg_attr(
        all(debug_assertions, feature = "openapi"),
        schema(value_type = Option<String>)
    )]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub max_downloads: i64,
}

/// Batch delete shares.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct BatchDeleteSharesReq {
    #[serde(default)]
    pub share_ids: Vec<i64>,
}
