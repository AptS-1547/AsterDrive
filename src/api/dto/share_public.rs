use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// Verify a share password.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct VerifyPasswordReq {
    pub password: String,
}

/// Query parameters for direct link downloads.
/// NOTE: The `force_download()` method is defined in `src/api/routes/share_public.rs`.
#[derive(Deserialize, Default)]
pub struct DirectLinkQuery {
    pub download: Option<String>,
}
