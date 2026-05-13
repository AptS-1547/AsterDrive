use crate::api::response::ApiErrorInfo;
use crate::types::DriverType;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteStorageCapabilities {
    pub protocol_version: String,
    pub supports_list: bool,
    pub supports_range_read: bool,
    pub supports_stream_upload: bool,
}

impl Default for RemoteStorageCapabilities {
    fn default() -> Self {
        Self {
            protocol_version: "v1".to_string(),
            supports_list: true,
            supports_range_read: true,
            supports_stream_upload: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RemoteStorageListResponse {
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteStorageObjectMetadata {
    pub size: u64,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteBindingSyncRequest {
    pub name: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteIngressProfileInfo {
    pub profile_key: String,
    pub name: String,
    pub driver_type: DriverType,
    pub endpoint: String,
    pub bucket: String,
    pub base_path: String,
    pub max_file_size: i64,
    pub is_default: bool,
    pub desired_revision: i64,
    pub applied_revision: i64,
    pub last_error: String,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "driver_type", rename_all = "lowercase")]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub enum RemoteCreateIngressProfileRequest {
    Local(RemoteCreateLocalIngressProfileRequest),
    S3(RemoteCreateS3IngressProfileRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteCreateLocalIngressProfileRequest {
    pub name: String,
    pub base_path: String,
    pub max_file_size: i64,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteCreateS3IngressProfileRequest {
    pub name: String,
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub base_path: String,
    pub max_file_size: i64,
    #[serde(default)]
    pub is_default: bool,
}

impl fmt::Debug for RemoteCreateS3IngressProfileRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteCreateS3IngressProfileRequest")
            .field("name", &self.name)
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .field("base_path", &self.base_path)
            .field("max_file_size", &self.max_file_size)
            .field("is_default", &self.is_default)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct RemoteUpdateIngressProfileRequest {
    pub name: Option<String>,
    pub driver_type: Option<DriverType>,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_path: Option<String>,
    pub max_file_size: Option<i64>,
    pub is_default: Option<bool>,
}

impl fmt::Debug for RemoteUpdateIngressProfileRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteUpdateIngressProfileRequest")
            .field("name", &self.name)
            .field("driver_type", &self.driver_type)
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field(
                "access_key",
                &self.access_key.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "secret_key",
                &self.secret_key.as_ref().map(|_| "<redacted>"),
            )
            .field("base_path", &self.base_path)
            .field("max_file_size", &self.max_file_size)
            .field("is_default", &self.is_default)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteStorageComposeRequest {
    pub target_key: String,
    pub part_keys: Vec<String>,
    pub expected_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteStorageComposeResponse {
    pub bytes_written: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiEnvelope<T> {
    pub(super) code: i32,
    pub(super) msg: String,
    pub(super) data: Option<T>,
    pub(super) error: Option<ApiErrorInfo>,
}
