use super::s3_config::normalize_s3_endpoint_and_bucket;
use crate::entities::storage_policy;
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::storage::driver::{BlobMetadata, StorageDriver, StoragePathVisitor};
use crate::storage::multipart::MultipartStorageDriver;
use crate::utils::numbers;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::operation::{RequestId, RequestIdExt};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use futures::Stream;
use http_body::{Frame, SizeHint};
use std::error::Error as StdError;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

pub struct S3Driver {
    client: Client,
    bucket: String,
    base_path: String,
}

const STREAM_UPLOAD_BUFFER_SIZE: usize = 64 * 1024;

struct SizedReaderBody<R> {
    stream: ReaderStream<R>,
    remaining: u64,
    finished: bool,
}

impl<R> SizedReaderBody<R>
where
    R: AsyncRead + Unpin,
{
    fn new(reader: R, size: u64) -> Self {
        Self {
            stream: ReaderStream::with_capacity(reader, STREAM_UPLOAD_BUFFER_SIZE),
            remaining: size,
            finished: false,
        }
    }
}

impl<R> http_body::Body for SizedReaderBody<R>
where
    R: AsyncRead + Unpin + Send + Sync + 'static,
{
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Self::Data>, Self::Error>>> {
        if self.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(chunk))) => {
                let chunk_len = numbers::usize_to_u64(chunk.len());
                if chunk_len > self.remaining {
                    self.finished = true;
                    return Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "upload stream exceeded declared size",
                    ))));
                }

                self.remaining -= chunk_len;
                Poll::Ready(Some(Ok(Frame::data(chunk))))
            }
            Poll::Ready(Some(Err(err))) => {
                self.finished = true;
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                self.finished = true;
                if self.remaining == 0 {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!(
                            "upload stream ended before declared size: {} bytes missing",
                            self.remaining
                        ),
                    ))))
                }
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.finished && self.remaining == 0
    }

    fn size_hint(&self) -> SizeHint {
        let mut hint = SizeHint::new();
        hint.set_exact(self.remaining);
        hint
    }
}

impl S3Driver {
    const ERROR_BODY_PREVIEW_LIMIT: usize = 512;

    fn rewrap_message_as_storage_error(err: AsterError) -> AsterError {
        AsterError::storage_driver_error(err.message().to_string())
    }

    pub fn new(policy: &storage_policy::Model) -> Result<Self> {
        let normalized = normalize_s3_endpoint_and_bucket(&policy.endpoint, &policy.bucket)
            .map_err(Self::rewrap_message_as_storage_error)?;

        let credentials = Credentials::new(
            &policy.access_key,
            &policy.secret_key,
            None,
            None,
            "aster-s3-driver",
        );

        let mut config_builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .credentials_provider(credentials)
            .force_path_style(true); // MinIO / R2 等需要

        // 自定义 endpoint（MinIO、R2、OSS 等）
        if !normalized.endpoint.is_empty() {
            config_builder = config_builder.endpoint_url(&normalized.endpoint);
        }

        let config = config_builder.build();
        let client = Client::from_conf(config);

        Ok(Self {
            client,
            bucket: normalized.bucket,
            base_path: policy.base_path.clone(),
        })
    }

    fn full_key(&self, path: &str) -> String {
        if self.base_path.is_empty() {
            path.trim_start_matches('/').to_string()
        } else {
            format!(
                "{}/{}",
                self.base_path.trim_end_matches('/'),
                path.trim_start_matches('/')
            )
        }
    }

    fn relative_key(&self, key: &str) -> String {
        if self.base_path.is_empty() {
            return key.trim_start_matches('/').to_string();
        }

        key.trim_start_matches(self.base_path.trim_end_matches('/'))
            .trim_start_matches('/')
            .to_string()
    }

    fn normalize_multipart_etag(etag: &str) -> String {
        let etag = etag.trim();
        if etag.starts_with('"') && etag.ends_with('"') && etag.len() >= 2 {
            etag.to_string()
        } else {
            format!("\"{etag}\"")
        }
    }

    fn error_chain(err: &dyn StdError) -> String {
        let mut parts = Vec::new();
        let mut current = Some(err);
        while let Some(err) = current {
            let message = err.to_string();
            if parts.last() != Some(&message) {
                parts.push(message);
            }
            current = err.source();
        }
        parts.join(": ")
    }

    fn truncate_for_log(text: &str, limit: usize) -> String {
        let mut result = String::new();
        for ch in text.chars().take(limit) {
            result.push(ch);
        }
        if text.chars().count() > limit {
            result.push_str("...");
        }
        result
    }

    fn extract_xml_tag(body: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{tag}>");
        let end_tag = format!("</{tag}>");
        let start = body.find(&start_tag)? + start_tag.len();
        let end = body[start..].find(&end_tag)? + start;
        let value = body[start..end].trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn raw_body_preview(body: &str) -> Option<String> {
        let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            None
        } else {
            Some(Self::truncate_for_log(
                &normalized,
                Self::ERROR_BODY_PREVIEW_LIMIT,
            ))
        }
    }

    fn format_sdk_error<E>(err: &SdkError<E>) -> String
    where
        E: StdError + ProvideErrorMetadata + Send + Sync + 'static,
    {
        let mut details = Vec::new();
        let mut code = err.code().map(str::to_string);
        let mut message = err.message().map(str::to_string);
        let mut request_id = err.request_id().map(str::to_string);
        let mut extended_request_id = err.extended_request_id().map(str::to_string);
        let mut http_status = None;
        let mut content_type = None;
        let mut raw_body = None;

        if let Some(raw) = err.raw_response() {
            http_status = Some(raw.status().as_u16());
            content_type = raw.headers().get("content-type").map(str::to_string);
            request_id =
                request_id.or_else(|| raw.headers().get("x-amz-request-id").map(str::to_string));
            extended_request_id =
                extended_request_id.or_else(|| raw.headers().get("x-amz-id-2").map(str::to_string));

            if let Some(bytes) = raw.body().bytes()
                && let Ok(body) = std::str::from_utf8(bytes)
            {
                code = code.or_else(|| Self::extract_xml_tag(body, "Code"));
                message = message.or_else(|| Self::extract_xml_tag(body, "Message"));
                request_id = request_id.or_else(|| Self::extract_xml_tag(body, "RequestId"));
                extended_request_id =
                    extended_request_id.or_else(|| Self::extract_xml_tag(body, "HostId"));
                raw_body = Self::raw_body_preview(body);
            }
        }

        let has_structured_error = code.is_some() || message.is_some();

        if let Some(http_status) = http_status {
            details.push(format!("http_status={http_status}"));
        }
        if let Some(code) = code {
            details.push(format!("code={code}"));
        }
        if let Some(message) = message {
            details.push(format!("message={message}"));
        }
        if let Some(request_id) = request_id {
            details.push(format!("request_id={request_id}"));
        }
        if let Some(extended_request_id) = extended_request_id {
            details.push(format!("extended_request_id={extended_request_id}"));
        }
        if let Some(content_type) = content_type {
            details.push(format!("content_type={content_type}"));
        }
        if !has_structured_error && let Some(raw_body) = raw_body {
            details.push(format!("raw_body={raw_body}"));
        }

        if details.is_empty() {
            Self::error_chain(err)
        } else {
            details.join(", ")
        }
    }

    fn map_sdk_error<E>(ctx: &str, err: SdkError<E>) -> AsterError
    where
        E: StdError + ProvideErrorMetadata + Send + Sync + 'static,
    {
        AsterError::storage_driver_error(format!("{ctx}: {}", Self::format_sdk_error(&err)))
    }
}

#[async_trait]
impl StorageDriver for S3Driver {
    async fn put(&self, path: &str, data: &[u8]) -> Result<String> {
        let key = self.full_key(path);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 put failed", err))?;
        Ok(path.to_string())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let key = self.full_key(path);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 get failed", err))?;

        let bytes = resp
            .body
            .collect()
            .await
            .map_aster_err_ctx("S3 read body failed", AsterError::storage_driver_error)?
            .into_bytes();

        Ok(bytes.to_vec())
    }

    async fn get_stream(&self, path: &str) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let key = self.full_key(path);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 get_stream failed", err))?;

        Ok(Box::new(resp.body.into_async_read()))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.full_key(path);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 delete failed", err))?;
        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let key = self.full_key(path);
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.as_service_error().map(|svc_err| svc_err.is_not_found()) == Some(true) {
                    Ok(false)
                } else {
                    Err(Self::map_sdk_error("S3 exists check failed", e))
                }
            }
        }
    }

    async fn metadata(&self, path: &str) -> Result<BlobMetadata> {
        let key = self.full_key(path);
        let resp = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 head failed", err))?;

        let size = resp
            .content_length
            .map(|value| numbers::i64_to_u64(value, "S3 content_length"))
            .transpose()
            .map_err(Self::rewrap_message_as_storage_error)?
            .unwrap_or(0);

        Ok(BlobMetadata {
            size,
            content_type: resp.content_type,
        })
    }

    async fn list_paths(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let full_prefix = prefix
            .map(|prefix| self.full_key(prefix))
            .unwrap_or_else(|| self.base_path.trim_end_matches('/').to_string());
        let mut continuation: Option<String> = None;
        let mut paths = Vec::new();

        loop {
            let mut request = self.client.list_objects_v2().bucket(&self.bucket);
            if !full_prefix.is_empty() {
                request = request.prefix(full_prefix.clone());
            }
            if let Some(token) = continuation.as_deref() {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|err| Self::map_sdk_error("S3 list_objects_v2 failed", err))?;

            for object in response.contents() {
                let Some(key) = object.key() else {
                    continue;
                };
                paths.push(self.relative_key(key));
            }

            let truncated = response.is_truncated().unwrap_or(false);
            continuation = response.next_continuation_token().map(ToOwned::to_owned);
            if !truncated || continuation.is_none() {
                break;
            }
        }

        paths.sort();
        Ok(paths)
    }

    async fn scan_paths(
        &self,
        prefix: Option<&str>,
        visitor: &mut dyn StoragePathVisitor,
    ) -> Result<()> {
        let full_prefix = prefix
            .map(|prefix| self.full_key(prefix))
            .unwrap_or_else(|| self.base_path.trim_end_matches('/').to_string());
        let mut continuation: Option<String> = None;

        loop {
            let mut request = self.client.list_objects_v2().bucket(&self.bucket);
            if !full_prefix.is_empty() {
                request = request.prefix(full_prefix.clone());
            }
            if let Some(token) = continuation.as_deref() {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|err| Self::map_sdk_error("S3 list_objects_v2 failed", err))?;

            for object in response.contents() {
                let Some(key) = object.key() else {
                    continue;
                };
                visitor.visit_path(self.relative_key(key))?;
            }

            let truncated = response.is_truncated().unwrap_or(false);
            continuation = response.next_continuation_token().map(ToOwned::to_owned);
            if !truncated || continuation.is_none() {
                break;
            }
        }

        Ok(())
    }

    async fn put_file(&self, storage_path: &str, local_path: &str) -> Result<String> {
        let key = self.full_key(storage_path);
        let body = ByteStream::from_path(local_path)
            .await
            .map_aster_err_ctx("S3 read file", AsterError::storage_driver_error)?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(body)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 put_file failed", err))?;
        Ok(storage_path.to_string())
    }

    async fn put_reader(
        &self,
        storage_path: &str,
        reader: Box<dyn AsyncRead + Unpin + Send + Sync>,
        size: i64,
    ) -> Result<String> {
        let key = self.full_key(storage_path);
        let content_length = numbers::i64_to_u64(size, "S3 put_reader content_length")?;
        let body = ByteStream::from_body_1_x(SizedReaderBody::new(reader, content_length));

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_length(size)
            .body(body)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 put_reader failed", err))?;

        Ok(storage_path.to_string())
    }

    async fn presigned_url(
        &self,
        path: &str,
        expires: Duration,
        options: crate::storage::driver::PresignedDownloadOptions,
    ) -> Result<Option<String>> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_aster_err_ctx("presign config", AsterError::storage_driver_error)?;

        let mut request = self.client.get_object().bucket(&self.bucket).key(&key);
        if let Some(cache_control) = options.response_cache_control {
            request = request.response_cache_control(cache_control);
        }
        if let Some(content_disposition) = options.response_content_disposition {
            request = request.response_content_disposition(content_disposition);
        }
        if let Some(content_type) = options.response_content_type {
            request = request.response_content_type(content_type);
        }

        let url = request
            .presigned(presign_config)
            .await
            .map_aster_err_ctx("S3 presigned URL failed", AsterError::storage_driver_error)?;

        Ok(Some(url.uri().to_string()))
    }

    async fn presigned_put_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_aster_err_ctx("presign config", AsterError::storage_driver_error)?;

        let url = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .presigned(presign_config)
            .await
            .map_aster_err_ctx("S3 presigned PUT failed", AsterError::storage_driver_error)?;

        Ok(Some(url.uri().to_string()))
    }

    async fn copy_object(&self, src_path: &str, dest_path: &str) -> Result<String> {
        let src_key = self.full_key(src_path);
        let dest_key = self.full_key(dest_path);
        let copy_source = format!("{}/{}", self.bucket, src_key);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(&copy_source)
            .key(&dest_key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 copy_object failed", err))?;

        Ok(dest_path.to_string())
    }

    fn as_multipart(&self) -> Option<&dyn crate::storage::multipart::MultipartStorageDriver> {
        Some(self)
    }
}

#[async_trait]
impl MultipartStorageDriver for S3Driver {
    async fn create_multipart_upload(&self, path: &str) -> Result<String> {
        let key = self.full_key(path);
        let resp = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 create_multipart_upload failed", err))?;

        resp.upload_id().map(|s| s.to_string()).ok_or_else(|| {
            AsterError::storage_driver_error("S3 multipart upload: missing upload_id")
        })
    }

    async fn presigned_upload_part_url(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        expires: Duration,
    ) -> Result<String> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_aster_err_ctx("presign config", AsterError::storage_driver_error)?;

        let url = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(upload_id)
            .part_number(part_number)
            .presigned(presign_config)
            .await
            .map_aster_err_ctx(
                "S3 presigned upload_part failed",
                AsterError::storage_driver_error,
            )?;

        Ok(url.uri().to_string())
    }

    async fn complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<(i32, String)>,
    ) -> Result<()> {
        use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};

        let completed_parts: Vec<CompletedPart> = parts
            .into_iter()
            .map(|(num, etag)| {
                CompletedPart::builder()
                    .part_number(num)
                    .e_tag(Self::normalize_multipart_etag(&etag))
                    .build()
            })
            .collect();

        let key = self.full_key(path);
        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(completed_parts))
                    .build(),
            )
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 complete_multipart_upload failed", err))?;

        Ok(())
    }

    async fn upload_multipart_part(
        &self,
        path: &str,
        upload_id: &str,
        part_number: i32,
        data: &[u8],
    ) -> Result<String> {
        let key = self.full_key(path);
        let resp = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 upload_part failed", err))?;

        resp.e_tag()
            .map(str::to_string)
            .ok_or_else(|| AsterError::storage_driver_error("S3 multipart upload: missing ETag"))
    }

    async fn abort_multipart_upload(&self, path: &str, upload_id: &str) -> Result<()> {
        let key = self.full_key(path);
        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(upload_id)
            .send()
            .await
            .map_err(|err| Self::map_sdk_error("S3 abort_multipart_upload failed", err))?;
        Ok(())
    }

    async fn list_uploaded_parts(&self, path: &str, upload_id: &str) -> Result<Vec<i32>> {
        let key = self.full_key(path);
        let mut part_numbers = Vec::new();
        let mut part_marker: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_parts()
                .bucket(&self.bucket)
                .key(&key)
                .upload_id(upload_id);
            if let Some(marker) = &part_marker {
                req = req.part_number_marker(marker.as_str());
            }

            let resp = req
                .send()
                .await
                .map_err(|err| Self::map_sdk_error("S3 list_parts failed", err))?;

            for part in resp.parts() {
                part_numbers.push(part.part_number.unwrap_or(0));
            }

            if resp.is_truncated() == Some(true) {
                part_marker = resp.next_part_number_marker().map(|s| s.to_string());
            } else {
                break;
            }
        }

        Ok(part_numbers)
    }
}

#[cfg(test)]
mod tests {
    use super::S3Driver;
    use crate::entities::storage_policy;
    use crate::errors::AsterError;
    use crate::storage::driver::StorageDriver;
    use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
    use aws_smithy_http_client::test_util::capture_request;
    use aws_smithy_types::body::SdkBody;

    fn mocked_driver(
        response: http::Response<SdkBody>,
    ) -> (
        S3Driver,
        aws_smithy_http_client::test_util::CaptureRequestReceiver,
    ) {
        let (http_client, request) = capture_request(Some(response));
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .http_client(http_client)
            .credentials_provider(Credentials::new(
                "test-access-key",
                "test-secret-key",
                None,
                None,
                "s3-unit-test",
            ))
            .region(Region::new("us-east-1"))
            .build();

        (
            S3Driver {
                client: aws_sdk_s3::Client::from_conf(config),
                bucket: "test-bucket".to_string(),
                base_path: String::new(),
            },
            request,
        )
    }

    fn assert_storage_driver_error(err: AsterError) {
        assert_eq!(err.code(), "E031");
        assert!(
            err.message().contains("http_status=404"),
            "expected raw HTTP status in '{}'",
            err.message()
        );
        assert!(
            err.message().contains("code=NoSuchBucket"),
            "expected S3 error code in '{}'",
            err.message()
        );
        assert!(
            err.message()
                .contains("message=The specified bucket does not exist"),
            "expected S3 error message in '{}'",
            err.message()
        );
        assert!(
            err.message().contains("request_id=req-123"),
            "expected S3 request_id in '{}'",
            err.message()
        );
        assert!(
            err.message().contains("extended_request_id=ext-456"),
            "expected S3 extended_request_id in '{}'",
            err.message()
        );
    }

    fn sample_policy(endpoint: &str, bucket: &str) -> storage_policy::Model {
        storage_policy::Model {
            id: 1,
            name: "S3".to_string(),
            driver_type: crate::types::DriverType::S3,
            endpoint: endpoint.to_string(),
            bucket: bucket.to_string(),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
            base_path: String::new(),
            max_file_size: 0,
            allowed_types: crate::types::StoredStoragePolicyAllowedTypes::empty(),
            options: crate::types::StoredStoragePolicyOptions::empty(),
            is_default: false,
            chunk_size: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn new_normalizes_r2_bucket_path() {
        let driver = S3Driver::new(&sample_policy(
            "https://demo-account.r2.cloudflarestorage.com/photos",
            "",
        ))
        .expect("normalized R2 driver");

        assert_eq!(driver.bucket, "photos");
    }

    #[test]
    fn new_maps_r2_validation_errors_to_storage_driver_errors() {
        let err = match S3Driver::new(&sample_policy("https://pub-demo.r2.dev", "photos")) {
            Ok(_) => panic!("public R2 endpoint should fail"),
            Err(err) => err,
        };

        assert_eq!(err.code(), "E031");
        assert!(
            err.message().contains("Cloudflare R2 endpoint"),
            "expected R2 validation context in '{}'",
            err.message()
        );
    }

    #[tokio::test]
    async fn put_surfaces_s3_service_error_details() {
        let response = http::Response::builder()
            .status(404)
            .header("x-amz-request-id", "req-123")
            .header("x-amz-id-2", "ext-456")
            .body(SdkBody::from(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <Error>
                    <Code>NoSuchBucket</Code>
                    <Message>The specified bucket does not exist</Message>
                    <RequestId>ignored-in-body</RequestId>
                </Error>"#,
            ))
            .expect("mocked response");
        let (driver, request) = mocked_driver(response);

        let err = driver.put("foo.txt", b"hello").await.unwrap_err();
        request.expect_request();

        assert_storage_driver_error(err);
    }

    #[tokio::test]
    async fn put_surfaces_raw_http_error_when_metadata_missing() {
        let response = http::Response::builder()
            .status(403)
            .header("content-type", "text/plain")
            .body(SdkBody::from("upstream denied this request"))
            .expect("mocked response");
        let (driver, request) = mocked_driver(response);

        let err = driver.put("foo.txt", b"hello").await.unwrap_err();
        request.expect_request();

        assert_eq!(err.code(), "E031");
        assert!(
            err.message().contains("http_status=403"),
            "expected raw HTTP status in '{}'",
            err.message()
        );
        assert!(
            err.message().contains("content_type=text/plain"),
            "expected content type in '{}'",
            err.message()
        );
        assert!(
            err.message()
                .contains("raw_body=upstream denied this request"),
            "expected raw body preview in '{}'",
            err.message()
        );
    }
}
