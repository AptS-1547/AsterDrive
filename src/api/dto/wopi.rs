//! `wopi` API DTO 定义。

use serde::Deserialize;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// Query parameters for WOPI file endpoints.
#[derive(Deserialize, Validate)]
#[cfg_attr(
    all(debug_assertions, feature = "openapi"),
    derive(IntoParams, ToSchema)
)]
pub struct WopiAccessQuery {
    pub access_token: Option<String>,
}
