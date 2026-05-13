use super::S3Driver;
use super::presigned::{MAX_PRESIGN_TTL, clamp_presign_ttl};
use crate::entities::storage_policy;
use crate::errors::AsterError;
use crate::storage::driver::StorageDriver;
use crate::storage::error::StorageErrorKind;
use crate::storage::multipart::MultipartStorageDriver;
use crate::types::{StoragePolicyOptions, serialize_storage_policy_options};
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_smithy_http_client::test_util::capture_request;
use aws_smithy_types::body::SdkBody;
use std::time::Duration;

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

fn assert_storage_driver_error(err: AsterError, expected_kind: StorageErrorKind) {
    assert_eq!(err.code(), "E031");
    assert_eq!(err.storage_error_kind(), Some(expected_kind));
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
        remote_node_id: None,
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

#[test]
fn new_applies_timeout_config_from_policy_options() {
    let mut policy = sample_policy("https://s3.example.test", "bucket");
    policy.options = serialize_storage_policy_options(&StoragePolicyOptions {
        s3_connect_timeout_secs: Some(9),
        s3_read_timeout_secs: Some(45),
        s3_operation_timeout_secs: Some(1_200),
        ..Default::default()
    })
    .expect("options should serialize");

    let driver = S3Driver::new(&policy).expect("driver should build with timeout config");
    let timeout_config = driver
        .client
        .config()
        .timeout_config()
        .expect("timeout config should be present");

    assert_eq!(
        timeout_config.connect_timeout(),
        Some(Duration::from_secs(9))
    );
    assert_eq!(timeout_config.read_timeout(), Some(Duration::from_secs(45)));
    assert_eq!(
        timeout_config.operation_timeout(),
        Some(Duration::from_secs(1_200))
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

    assert_storage_driver_error(err, StorageErrorKind::Misconfigured);
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
    assert_eq!(err.storage_error_kind(), Some(StorageErrorKind::Permission));
}

#[tokio::test]
async fn abort_multipart_upload_maps_no_such_upload_to_not_found() {
    let response = http::Response::builder()
        .status(404)
        .header("x-amz-request-id", "req-404")
        .body(SdkBody::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <Error>
                    <Code>NoSuchUpload</Code>
                    <Message>The specified multipart upload does not exist</Message>
                </Error>"#,
        ))
        .expect("mocked response");
    let (driver, request) = mocked_driver(response);

    let err = driver
        .abort_multipart_upload("foo.txt", "upload-1")
        .await
        .unwrap_err();
    request.expect_request();

    assert_eq!(err.code(), "E031");
    assert_eq!(err.storage_error_kind(), Some(StorageErrorKind::NotFound));
}

#[tokio::test]
async fn copy_object_url_encodes_source_key() {
    let response = http::Response::builder()
        .status(200)
        .body(SdkBody::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <CopyObjectResult><ETag>"abc"</ETag></CopyObjectResult>"#,
        ))
        .expect("mocked response");
    let (driver, request) = mocked_driver(response);

    driver
        .copy_object("folder with space/中文 file+1.txt", "dest/key")
        .await
        .expect("copy should succeed");

    let captured = request.expect_request();
    let copy_source = captured
        .headers()
        .get("x-amz-copy-source")
        .expect("copy-source header");
    // 空格 → %20，中文 → UTF-8 percent-encoded，`+` → %2B
    assert!(
        copy_source.contains("%20"),
        "expected space encoded in '{copy_source}'"
    );
    assert!(
        copy_source.contains("%2B"),
        "expected '+' encoded in '{copy_source}'"
    );
    assert!(
        !copy_source.contains(' '),
        "raw space should not remain in '{copy_source}'"
    );
    // bucket 与 key 之间的 `/` 必须保留为分隔符
    assert!(
        copy_source.starts_with("test-bucket/"),
        "bucket prefix missing in '{copy_source}'"
    );
}

#[tokio::test]
async fn get_range_sends_native_range_header() {
    let response = http::Response::builder()
        .status(206)
        .body(SdkBody::from("world"))
        .expect("mocked response");
    let (driver, request) = mocked_driver(response);

    driver
        .get_range("obj", 7, Some(5))
        .await
        .expect("range should succeed");

    let captured = request.expect_request();
    let range = captured
        .headers()
        .get("range")
        .expect("Range header must be sent");
    // HTTP Range 闭区间，7..=11
    assert_eq!(range, "bytes=7-11");
}

#[tokio::test]
async fn get_range_without_length_sends_open_ended_range() {
    let response = http::Response::builder()
        .status(206)
        .body(SdkBody::from("tail"))
        .expect("mocked response");
    let (driver, request) = mocked_driver(response);

    driver
        .get_range("obj", 100, None)
        .await
        .expect("open-ended range should succeed");

    let captured = request.expect_request();
    let range = captured.headers().get("range").expect("Range header");
    assert_eq!(range, "bytes=100-");
}

#[test]
fn clamp_presign_ttl_caps_at_max() {
    let clamped = clamp_presign_ttl(std::time::Duration::from_secs(7 * 24 * 3600), "t");
    assert_eq!(clamped, MAX_PRESIGN_TTL);
}

#[test]
fn clamp_presign_ttl_passes_through_when_in_range() {
    let req = std::time::Duration::from_secs(60);
    assert_eq!(clamp_presign_ttl(req, "t"), req);
}

#[test]
fn clamp_presign_ttl_replaces_zero_with_max() {
    let clamped = clamp_presign_ttl(std::time::Duration::ZERO, "t");
    assert_eq!(clamped, MAX_PRESIGN_TTL);
}
