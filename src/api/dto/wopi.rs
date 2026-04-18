//! `wopi` API DTO 定义。

use serde::Deserialize;
use validator::Validate;

/// Query parameters for WOPI file endpoints.
#[derive(Deserialize, Validate)]
pub struct WopiAccessQuery {
    #[validate(custom(function = "crate::api::dto::validation::validate_non_blank"))]
    pub access_token: String,
}
