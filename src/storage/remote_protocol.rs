//! 远端节点内部对象协议与客户端。

use crate::api::error_code::ErrorCode;
use crate::api::response::ApiErrorInfo;
use crate::errors::{AsterError, Result, precondition_failed_with_subcode};
use crate::storage::driver::{BlobMetadata, PresignedDownloadOptions};
use crate::storage::error::{
    StorageErrorKind, storage_driver_error, storage_driver_error_with_subcode,
};
use futures::TryStreamExt;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{AsciiSet, CONTROLS, percent_encode};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio_util::io::{ReaderStream, StreamReader};
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::ToSchema;

const STORAGE_KEY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'{')
    .add(b'}');

const DEFAULT_REMOTE_CONNECT_TIMEOUT_SECS: u64 = 5;
const DEFAULT_REMOTE_READ_TIMEOUT_SECS: u64 = 30;
const DEFAULT_REMOTE_OPERATION_TIMEOUT_SECS: u64 = 60 * 60;

static REMOTE_HTTP_CLIENT: LazyLock<std::result::Result<reqwest::Client, String>> =
    LazyLock::new(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_REMOTE_CONNECT_TIMEOUT_SECS))
            .read_timeout(Duration::from_secs(DEFAULT_REMOTE_READ_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_REMOTE_OPERATION_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("build remote HTTP client: {e}"))
    });

pub const INTERNAL_STORAGE_BASE_PATH: &str = "/api/v1/internal/storage";
pub const INTERNAL_AUTH_ACCESS_KEY_HEADER: &str = "x-aster-access-key";
pub const INTERNAL_AUTH_TIMESTAMP_HEADER: &str = "x-aster-timestamp";
pub const INTERNAL_AUTH_NONCE_HEADER: &str = "x-aster-nonce";
pub const INTERNAL_AUTH_SIGNATURE_HEADER: &str = "x-aster-signature";
pub const INTERNAL_AUTH_SKEW_SECS: i64 = 300;
pub const INTERNAL_AUTH_NONCE_TTL_SECS: u64 = 300;
pub const PRESIGNED_AUTH_ACCESS_KEY_QUERY: &str = "aster_access_key";
pub const PRESIGNED_AUTH_EXPIRES_QUERY: &str = "aster_expires";
pub const PRESIGNED_AUTH_SIGNATURE_QUERY: &str = "aster_signature";
pub const PRESIGNED_RESPONSE_CACHE_CONTROL_QUERY: &str = "response-cache-control";
pub const PRESIGNED_RESPONSE_CONTENT_DISPOSITION_QUERY: &str = "response-content-disposition";
pub const PRESIGNED_RESPONSE_CONTENT_TYPE_QUERY: &str = "response-content-type";

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
    pub namespace: String,
    pub is_enabled: bool,
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
struct ApiEnvelope<T> {
    code: i32,
    msg: String,
    data: Option<T>,
    error: Option<ApiErrorInfo>,
}

pub fn normalize_remote_base_url(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut url = reqwest::Url::parse(trimmed)
        .map_err(|e| AsterError::validation_error(format!("invalid remote node base_url: {e}")))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AsterError::validation_error(format!(
                "remote node base_url must use http/https, got '{other}'"
            )));
        }
    }
    url.set_query(None);
    url.set_fragment(None);
    while url.path().ends_with('/') && url.path() != "/" {
        let next = url.path().trim_end_matches('/').to_string();
        url.set_path(&next);
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub fn sign_internal_request(
    secret_key: &str,
    method: &str,
    path_and_query: &str,
    timestamp: i64,
    nonce: &str,
    content_length: Option<u64>,
) -> String {
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}",
        method,
        path_and_query,
        timestamp,
        nonce,
        content_length
            .map(|value| value.to_string())
            .unwrap_or_default()
    );
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn sign_presigned_request(
    secret_key: &str,
    method: &str,
    request_target: &str,
    access_key: &str,
    expires_at: i64,
) -> String {
    let canonical = format!(
        "{}\n{}\n{}\n{}",
        method, request_target, access_key, expires_at
    );
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[derive(Clone)]
pub struct RemoteStorageClient {
    base_url: String,
    access_key: String,
    secret_key: String,
    client: reqwest::Client,
}

impl RemoteStorageClient {
    pub fn new(base_url: &str, access_key: &str, secret_key: &str) -> Result<Self> {
        let normalized_base_url = normalize_remote_base_url(base_url)?;
        if normalized_base_url.is_empty() {
            return Err(AsterError::validation_error(
                "remote node base_url is required for outbound access",
            ));
        }
        if access_key.trim().is_empty() {
            return Err(AsterError::validation_error(
                "remote node access_key cannot be empty",
            ));
        }
        if secret_key.trim().is_empty() {
            return Err(AsterError::validation_error(
                "remote node secret_key cannot be empty",
            ));
        }

        let client = remote_http_client()?;

        Ok(Self {
            base_url: normalized_base_url,
            access_key: access_key.trim().to_string(),
            secret_key: secret_key.to_string(),
            client,
        })
    }

    pub async fn probe_capabilities(&self) -> Result<RemoteStorageCapabilities> {
        let url = self.url_for_path(&format!("{INTERNAL_STORAGE_BASE_PATH}/capabilities"))?;
        let response = self
            .signed_request(Method::GET, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let body = ensure_success(response, "probe remote storage capabilities").await?;
        let envelope: ApiEnvelope<RemoteStorageCapabilities> = serde_json::from_slice(&body)
            .map_err(|e| {
                storage_driver_error(
                    StorageErrorKind::Misconfigured,
                    format!("decode remote storage capabilities response: {e}"),
                )
            })?;
        if envelope.code != 0 {
            return Err(storage_driver_error(
                remote_api_error_kind(envelope.code).unwrap_or(StorageErrorKind::Unknown),
                format!("remote storage capabilities failed: {}", envelope.msg),
            ));
        }
        envelope.data.ok_or_else(|| {
            storage_driver_error(
                StorageErrorKind::Misconfigured,
                "remote storage capabilities response missing data",
            )
        })
    }

    pub async fn put_bytes(&self, key: &str, data: &[u8]) -> Result<()> {
        let url = self.object_url(key)?;
        let content_length = u64::try_from(data.len()).map_err(|_| {
            storage_driver_error(
                StorageErrorKind::Precondition,
                "remote upload body length overflow",
            )
        })?;
        let response = self
            .signed_request(Method::PUT, url, Some(content_length))
            .body(data.to_vec())
            .send()
            .await
            .map_err(map_reqwest_error)?;
        ensure_success_without_body(response, "put remote storage object").await
    }

    pub async fn put_reader(
        &self,
        key: &str,
        reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: u64,
    ) -> Result<()> {
        let url = self.object_url(key)?;
        let stream = ReaderStream::new(reader).map_err(std::io::Error::other);
        let response = self
            .signed_request(Method::PUT, url, Some(size))
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await
            .map_err(map_reqwest_error)?;
        ensure_success_without_body(response, "stream put remote storage object").await
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Vec<u8>> {
        let url = self.object_url(key)?;
        let response = self
            .signed_request(Method::GET, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        ensure_success(response, "get remote storage object").await
    }

    pub async fn get_stream(
        &self,
        key: &str,
        offset: Option<u64>,
        length: Option<u64>,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let mut url = self.object_url(key)?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(offset) = offset {
                query.append_pair("offset", &offset.to_string());
            }
            if let Some(length) = length {
                query.append_pair("length", &length.to_string());
            }
        }

        let response = self
            .signed_request(Method::GET, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let response = ensure_success_response(response, "stream remote storage object").await?;
        let stream = response
            .bytes_stream()
            .map_err(|error| std::io::Error::other(error.to_string()));
        Ok(Box::new(StreamReader::new(stream)))
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let url = self.object_url(key)?;
        let response = self
            .signed_request(Method::DELETE, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        ensure_success_without_body(response, "delete remote storage object").await
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let url = self.object_url(key)?;
        let response = self
            .signed_request(Method::HEAD, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        match response.status() {
            reqwest::StatusCode::OK => Ok(true),
            reqwest::StatusCode::NOT_FOUND => Ok(false),
            _ => {
                let error =
                    build_remote_status_error(response, "head remote storage object", true).await;
                Err(error)
            }
        }
    }

    pub async fn metadata(&self, key: &str) -> Result<BlobMetadata> {
        let url = self.object_metadata_url(key)?;
        let response = self
            .signed_request(Method::GET, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let body = ensure_success(response, "get remote storage metadata").await?;
        let envelope: ApiEnvelope<RemoteStorageObjectMetadata> = serde_json::from_slice(&body)
            .map_err(|e| {
                storage_driver_error(
                    StorageErrorKind::Misconfigured,
                    format!("decode remote storage metadata response: {e}"),
                )
            })?;
        if envelope.code != 0 {
            return Err(storage_driver_error(
                remote_api_error_kind(envelope.code).unwrap_or(StorageErrorKind::Unknown),
                format!("remote storage metadata failed: {}", envelope.msg),
            ));
        }
        let metadata = envelope.data.ok_or_else(|| {
            storage_driver_error(
                StorageErrorKind::Misconfigured,
                "remote storage metadata response missing data",
            )
        })?;
        Ok(BlobMetadata {
            size: metadata.size,
            content_type: metadata.content_type,
        })
    }

    pub async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut url = self.url_for_path(&format!("{INTERNAL_STORAGE_BASE_PATH}/objects"))?;
        if let Some(prefix) = prefix.filter(|value| !value.is_empty()) {
            url.query_pairs_mut().append_pair("prefix", prefix);
        }
        let response = self
            .signed_request(Method::GET, url, None)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let body = ensure_success(response, "list remote storage objects").await?;
        let envelope: ApiEnvelope<RemoteStorageListResponse> = serde_json::from_slice(&body)
            .map_err(|e| {
                storage_driver_error(
                    StorageErrorKind::Misconfigured,
                    format!("decode remote storage list response: {e}"),
                )
            })?;
        if envelope.code != 0 {
            return Err(storage_driver_error(
                remote_api_error_kind(envelope.code).unwrap_or(StorageErrorKind::Unknown),
                format!("remote storage list failed: {}", envelope.msg),
            ));
        }
        Ok(envelope.data.unwrap_or_default().items)
    }

    pub async fn sync_binding(&self, binding: &RemoteBindingSyncRequest) -> Result<()> {
        let url = self.url_for_path(&format!("{INTERNAL_STORAGE_BASE_PATH}/binding"))?;
        let body = serde_json::to_vec(binding).map_err(|e| {
            storage_driver_error(
                StorageErrorKind::Unknown,
                format!("encode remote binding sync request: {e}"),
            )
        })?;
        let content_length = u64::try_from(body.len()).map_err(|_| {
            storage_driver_error(
                StorageErrorKind::Precondition,
                "remote binding sync body length overflow",
            )
        })?;
        let response = self
            .signed_request(Method::PUT, url, Some(content_length))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        ensure_success_without_body(response, "sync remote binding state").await
    }

    pub fn presigned_put_url(&self, key: &str, expires: Duration) -> Result<String> {
        let mut url = self.object_url(key)?;
        let request_target = presigned_request_target(&url);
        let expires_at = presigned_expires_at(expires)?;
        let signature = sign_presigned_request(
            &self.secret_key,
            Method::PUT.as_str(),
            &request_target,
            &self.access_key,
            expires_at,
        );
        url.query_pairs_mut()
            .append_pair(PRESIGNED_AUTH_ACCESS_KEY_QUERY, &self.access_key)
            .append_pair(PRESIGNED_AUTH_EXPIRES_QUERY, &expires_at.to_string())
            .append_pair(PRESIGNED_AUTH_SIGNATURE_QUERY, &signature);

        Ok(url.to_string())
    }

    pub fn presigned_url(
        &self,
        key: &str,
        expires: Duration,
        options: PresignedDownloadOptions,
    ) -> Result<String> {
        let mut url = self.object_url(key)?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(cache_control) = options.response_cache_control.as_deref() {
                query.append_pair(PRESIGNED_RESPONSE_CACHE_CONTROL_QUERY, cache_control);
            }
            if let Some(content_disposition) = options.response_content_disposition.as_deref() {
                query.append_pair(
                    PRESIGNED_RESPONSE_CONTENT_DISPOSITION_QUERY,
                    content_disposition,
                );
            }
            if let Some(content_type) = options.response_content_type.as_deref() {
                query.append_pair(PRESIGNED_RESPONSE_CONTENT_TYPE_QUERY, content_type);
            }
        }

        let request_target = presigned_request_target(&url);
        let expires_at = presigned_expires_at(expires)?;
        let signature = sign_presigned_request(
            &self.secret_key,
            Method::GET.as_str(),
            &request_target,
            &self.access_key,
            expires_at,
        );
        url.query_pairs_mut()
            .append_pair(PRESIGNED_AUTH_ACCESS_KEY_QUERY, &self.access_key)
            .append_pair(PRESIGNED_AUTH_EXPIRES_QUERY, &expires_at.to_string())
            .append_pair(PRESIGNED_AUTH_SIGNATURE_QUERY, &signature);

        Ok(url.to_string())
    }

    pub async fn compose_objects(
        &self,
        target_key: &str,
        part_keys: Vec<String>,
        expected_size: i64,
    ) -> Result<RemoteStorageComposeResponse> {
        let url = self.url_for_path(&format!("{INTERNAL_STORAGE_BASE_PATH}/compose"))?;
        let body = serde_json::to_vec(&RemoteStorageComposeRequest {
            target_key: target_key.to_string(),
            part_keys,
            expected_size,
        })
        .map_err(|e| {
            storage_driver_error(
                StorageErrorKind::Unknown,
                format!("encode remote compose request: {e}"),
            )
        })?;
        let content_length = u64::try_from(body.len()).map_err(|_| {
            storage_driver_error(
                StorageErrorKind::Precondition,
                "remote compose body length overflow",
            )
        })?;
        let response = self
            .signed_request(Method::POST, url, Some(content_length))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let body = ensure_success(response, "compose remote storage objects").await?;
        let envelope: ApiEnvelope<RemoteStorageComposeResponse> = serde_json::from_slice(&body)
            .map_err(|e| {
                storage_driver_error(
                    StorageErrorKind::Misconfigured,
                    format!("decode remote storage compose response: {e}"),
                )
            })?;
        if envelope.code != 0 {
            return Err(storage_driver_error(
                remote_api_error_kind(envelope.code).unwrap_or(StorageErrorKind::Unknown),
                format!("remote storage compose failed: {}", envelope.msg),
            ));
        }
        envelope.data.ok_or_else(|| {
            storage_driver_error(
                StorageErrorKind::Misconfigured,
                "remote storage compose response missing data",
            )
        })
    }

    fn signed_request(
        &self,
        method: Method,
        url: reqwest::Url,
        content_length: Option<u64>,
    ) -> reqwest::RequestBuilder {
        let timestamp = chrono::Utc::now().timestamp();
        let nonce = uuid::Uuid::new_v4().to_string();
        let path_and_query = if let Some(query) = url.query() {
            format!("{}?{query}", url.path())
        } else {
            url.path().to_string()
        };
        let signature = sign_internal_request(
            &self.secret_key,
            method.as_str(),
            &path_and_query,
            timestamp,
            &nonce,
            content_length,
        );

        let mut request = self
            .client
            .request(method, url)
            .header(INTERNAL_AUTH_ACCESS_KEY_HEADER, &self.access_key)
            .header(INTERNAL_AUTH_TIMESTAMP_HEADER, timestamp.to_string())
            .header(INTERNAL_AUTH_NONCE_HEADER, nonce)
            .header(INTERNAL_AUTH_SIGNATURE_HEADER, signature);
        if let Some(content_length) = content_length {
            request = request.header(reqwest::header::CONTENT_LENGTH, content_length);
        }
        request
    }

    fn url_for_path(&self, path: &str) -> Result<reqwest::Url> {
        let joined = format!("{}{}", self.base_url, path);
        reqwest::Url::parse(&joined).map_err(|e| {
            storage_driver_error(
                StorageErrorKind::Misconfigured,
                format!("build remote storage url: {e}"),
            )
        })
    }

    fn object_url(&self, key: &str) -> Result<reqwest::Url> {
        let key = key.trim_start_matches('/');
        let encoded_key = percent_encode(key.as_bytes(), STORAGE_KEY_ENCODE_SET).to_string();
        self.url_for_path(&format!(
            "{INTERNAL_STORAGE_BASE_PATH}/objects/{encoded_key}"
        ))
    }

    fn object_metadata_url(&self, key: &str) -> Result<reqwest::Url> {
        let key = key.trim_start_matches('/');
        let encoded_key = percent_encode(key.as_bytes(), STORAGE_KEY_ENCODE_SET).to_string();
        self.url_for_path(&format!(
            "{INTERNAL_STORAGE_BASE_PATH}/objects/{encoded_key}/metadata"
        ))
    }
}

fn remote_http_client() -> Result<reqwest::Client> {
    REMOTE_HTTP_CLIENT
        .as_ref()
        .cloned()
        .map_err(|message| storage_driver_error(StorageErrorKind::Misconfigured, message.clone()))
}

fn presigned_expires_at(expires: Duration) -> Result<i64> {
    let expires_secs = i64::try_from(expires.as_secs()).map_err(|_| {
        storage_driver_error(
            StorageErrorKind::Precondition,
            "remote presigned URL expiry exceeds i64 range",
        )
    })?;
    if expires_secs <= 0 {
        return Err(storage_driver_error(
            StorageErrorKind::Precondition,
            "remote presigned URL expiry must be positive",
        ));
    }

    chrono::Utc::now()
        .timestamp()
        .checked_add(expires_secs)
        .ok_or_else(|| {
            storage_driver_error(
                StorageErrorKind::Precondition,
                "remote presigned URL expiry overflow",
            )
        })
}

fn presigned_request_target(url: &reqwest::Url) -> String {
    if let Some(query) = url.query() {
        format!("{}?{query}", url.path())
    } else {
        url.path().to_string()
    }
}

fn map_reqwest_error(error: reqwest::Error) -> AsterError {
    if error.is_timeout() {
        storage_driver_error(
            StorageErrorKind::Transient,
            format!("remote storage request timed out: {error}"),
        )
    } else {
        storage_driver_error(
            StorageErrorKind::Transient,
            format!("remote storage request failed: {error}"),
        )
    }
}

async fn ensure_success(response: reqwest::Response, context: &str) -> Result<Vec<u8>> {
    let response = ensure_success_response(response, context).await?;
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(map_reqwest_error)
}

async fn ensure_success_without_body(response: reqwest::Response, context: &str) -> Result<()> {
    ensure_success_response(response, context).await?;
    Ok(())
}

async fn ensure_success_response(
    response: reqwest::Response,
    context: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(build_remote_status_error(response, context, false).await)
    }
}

async fn build_remote_status_error(
    response: reqwest::Response,
    context: &str,
    not_found_as_record: bool,
) -> AsterError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let envelope = serde_json::from_str::<ApiEnvelope<serde_json::Value>>(&body).ok();
    let remote_code = envelope.as_ref().map(|value| value.code);
    let remote_subcode = envelope
        .as_ref()
        .and_then(|value| value.error.as_ref())
        .and_then(|value| value.subcode.as_deref());
    let remote_message = envelope
        .as_ref()
        .map(|envelope| envelope.msg.as_str())
        .filter(|msg| !msg.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| body.trim().to_string());
    let message = if remote_message.is_empty() {
        format!("{context}: remote node returned HTTP {status}")
    } else {
        format!("{context}: {remote_message}")
    };
    if let Some(error) = remote_code.and_then(|code| remote_api_error(code, &message)) {
        return error;
    }
    let kind = remote_code
        .and_then(remote_api_error_kind)
        .unwrap_or_else(|| remote_status_error_kind(status));

    match status {
        reqwest::StatusCode::NOT_FOUND if not_found_as_record => {
            AsterError::record_not_found("remote storage object not found")
        }
        reqwest::StatusCode::PRECONDITION_FAILED => remote_subcode
            .map(|subcode| precondition_failed_with_subcode(subcode, message.clone()))
            .unwrap_or_else(|| AsterError::precondition_failed(message)),
        _ => remote_subcode
            .map(|subcode| storage_driver_error_with_subcode(kind, subcode, message.clone()))
            .unwrap_or_else(|| storage_driver_error(kind, message)),
    }
}

fn remote_api_error(code: i32, message: &str) -> Option<AsterError> {
    match code {
        code if code == ErrorCode::StorageQuotaExceeded as i32 => {
            Some(AsterError::storage_quota_exceeded(message.to_string()))
        }
        _ => None,
    }
}

fn remote_api_error_kind(code: i32) -> Option<StorageErrorKind> {
    match code {
        code if code == ErrorCode::BadRequest as i32 => Some(StorageErrorKind::Misconfigured),
        code if code == ErrorCode::StoragePolicyNotFound as i32
            || code == ErrorCode::StorageMisconfigured as i32 =>
        {
            Some(StorageErrorKind::Misconfigured)
        }
        code if code == ErrorCode::NotFound as i32
            || code == ErrorCode::FileNotFound as i32
            || code == ErrorCode::UploadSessionNotFound as i32
            || code == ErrorCode::StorageObjectNotFound as i32 =>
        {
            Some(StorageErrorKind::NotFound)
        }
        code if code == ErrorCode::RateLimited as i32
            || code == ErrorCode::StorageRateLimited as i32 =>
        {
            Some(StorageErrorKind::RateLimited)
        }
        code if code == ErrorCode::AuthFailed as i32
            || code == ErrorCode::TokenExpired as i32
            || code == ErrorCode::TokenInvalid as i32
            || code == ErrorCode::StorageAuthFailed as i32 =>
        {
            Some(StorageErrorKind::Auth)
        }
        code if code == ErrorCode::Forbidden as i32
            || code == ErrorCode::StoragePermissionDenied as i32 =>
        {
            Some(StorageErrorKind::Permission)
        }
        code if code == ErrorCode::PreconditionFailed as i32 => {
            Some(StorageErrorKind::Precondition)
        }
        code if code == ErrorCode::StoragePreconditionFailed as i32 => {
            Some(StorageErrorKind::Precondition)
        }
        code if code == ErrorCode::UnsupportedDriver as i32
            || code == ErrorCode::StorageOperationUnsupported as i32 =>
        {
            Some(StorageErrorKind::Unsupported)
        }
        code if code == ErrorCode::StorageTransientFailure as i32 => {
            Some(StorageErrorKind::Transient)
        }
        code if code == ErrorCode::StorageDriverError as i32 => Some(StorageErrorKind::Unknown),
        _ => None,
    }
}

fn remote_status_error_kind(status: reqwest::StatusCode) -> StorageErrorKind {
    match status {
        reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
            StorageErrorKind::Misconfigured
        }
        reqwest::StatusCode::UNAUTHORIZED => StorageErrorKind::Auth,
        reqwest::StatusCode::FORBIDDEN => StorageErrorKind::Permission,
        reqwest::StatusCode::NOT_FOUND => StorageErrorKind::NotFound,
        reqwest::StatusCode::CONFLICT | reqwest::StatusCode::PRECONDITION_FAILED => {
            StorageErrorKind::Precondition
        }
        reqwest::StatusCode::METHOD_NOT_ALLOWED | reqwest::StatusCode::NOT_IMPLEMENTED => {
            StorageErrorKind::Unsupported
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => StorageErrorKind::RateLimited,
        status if status.is_server_error() => StorageErrorKind::Transient,
        _ => StorageErrorKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_api_error_kind_maps_auth_codes() {
        assert_eq!(
            remote_api_error_kind(ErrorCode::AuthFailed as i32),
            Some(StorageErrorKind::Auth)
        );
        assert_eq!(
            remote_api_error_kind(ErrorCode::TokenExpired as i32),
            Some(StorageErrorKind::Auth)
        );
    }

    #[test]
    fn remote_api_error_kind_maps_unsupported_driver() {
        assert_eq!(
            remote_api_error_kind(ErrorCode::UnsupportedDriver as i32),
            Some(StorageErrorKind::Unsupported)
        );
        assert_eq!(
            remote_api_error_kind(ErrorCode::StorageOperationUnsupported as i32),
            Some(StorageErrorKind::Unsupported)
        );
    }

    #[test]
    fn remote_status_error_kind_maps_rate_limit_and_server_errors() {
        assert_eq!(
            remote_status_error_kind(reqwest::StatusCode::TOO_MANY_REQUESTS),
            StorageErrorKind::RateLimited
        );
        assert_eq!(
            remote_status_error_kind(reqwest::StatusCode::SERVICE_UNAVAILABLE),
            StorageErrorKind::Transient
        );
    }

    #[test]
    fn remote_api_error_maps_storage_quota_exceeded() {
        let err = remote_api_error(
            ErrorCode::StorageQuotaExceeded as i32,
            "put remote storage object: quota exceeded",
        )
        .expect("quota error should map");
        assert!(matches!(err, AsterError::StorageQuotaExceeded(_)));
        assert_eq!(err.message(), "put remote storage object: quota exceeded");
    }
}
