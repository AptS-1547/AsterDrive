use serde::{Deserialize, Serialize};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

/// WebDAV account settings for the current user.
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct WebdavSettingsInfo {
    pub prefix: String,
    pub endpoint: String,
}

/// Test WebDAV credentials.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct TestConnectionReq {
    pub username: String,
    pub password: String,
}

/// Create a new WebDAV sub-account.
#[derive(Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct CreateWebdavAccountReq {
    pub username: String,
    pub password: Option<String>,
    pub root_folder_id: Option<i64>,
}
