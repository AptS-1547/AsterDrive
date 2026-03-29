use super::driver::{BlobMetadata, StorageDriver};
use crate::entities::storage_policy;
use crate::errors::{AsterError, MapAsterErr, Result};
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::operation::{RequestId, RequestIdExt};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use std::error::Error as StdError;
use std::time::Duration;
use tokio::io::AsyncRead;

pub struct S3Driver {
    client: Client,
    bucket: String,
    base_path: String,
}

impl S3Driver {
    const ERROR_BODY_PREVIEW_LIMIT: usize = 512;

    pub fn new(policy: &storage_policy::Model) -> Result<Self> {
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
        if !policy.endpoint.is_empty() {
            config_builder = config_builder.endpoint_url(&policy.endpoint);
        }

        let config = config_builder.build();
        let client = Client::from_conf(config);

        let bucket = if policy.bucket.is_empty() {
            return Err(AsterError::storage_driver_error(
                "S3 bucket name is required",
            ));
        } else {
            policy.bucket.clone()
        };

        Ok(Self {
            client,
            bucket,
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

        Ok(BlobMetadata {
            size: resp.content_length.unwrap_or(0) as u64,
            content_type: resp.content_type,
        })
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

    async fn presigned_url(&self, path: &str, expires: Duration) -> Result<Option<String>> {
        let key = self.full_key(path);
        let presign_config = PresigningConfig::builder()
            .expires_in(expires)
            .build()
            .map_aster_err_ctx("presign config", AsterError::storage_driver_error)?;

        let url = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
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

    // ── S3 Multipart Upload ──────────────────────────────────────────

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
