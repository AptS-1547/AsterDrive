//! `wopi` API DTO 定义。

use serde::Deserialize;

/// Query parameters for WOPI file endpoints.
#[derive(Deserialize)]
pub struct WopiAccessQuery {
    pub access_token: String,
}
