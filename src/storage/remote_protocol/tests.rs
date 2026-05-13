use super::errors::{
    build_remote_status_error_from_parts, remote_api_error, remote_api_error_kind,
    remote_status_error_kind,
};
use super::*;
use crate::api::error_code::ErrorCode;
use crate::errors::AsterError;
use crate::storage::error::StorageErrorKind;
use crate::types::DriverType;

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

#[test]
fn s3_ingress_profile_create_debug_redacts_credentials() {
    let request = RemoteCreateS3IngressProfileRequest {
        name: "s3".to_string(),
        endpoint: "https://s3.example.com".to_string(),
        bucket: "bucket-a".to_string(),
        access_key: "plain-access-key".to_string(),
        secret_key: "plain-secret-key".to_string(),
        base_path: "ingress".to_string(),
        max_file_size: 1024,
        is_default: true,
    };

    let rendered = format!("{request:?}");
    assert!(rendered.contains("access_key"));
    assert!(rendered.contains("secret_key"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("plain-access-key"));
    assert!(!rendered.contains("plain-secret-key"));
}

#[test]
fn ingress_profile_update_debug_redacts_optional_credentials() {
    let request = RemoteUpdateIngressProfileRequest {
        name: Some("s3".to_string()),
        driver_type: Some(DriverType::S3),
        endpoint: Some("https://s3.example.com".to_string()),
        bucket: Some("bucket-a".to_string()),
        access_key: Some("plain-access-key".to_string()),
        secret_key: Some("plain-secret-key".to_string()),
        base_path: Some("ingress".to_string()),
        max_file_size: Some(1024),
        is_default: Some(true),
    };

    let rendered = format!("{request:?}");
    assert!(rendered.contains("access_key"));
    assert!(rendered.contains("secret_key"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("plain-access-key"));
    assert!(!rendered.contains("plain-secret-key"));
}

#[test]
fn ingress_profile_url_encodes_path_separators_inside_profile_key() {
    let client = RemoteStorageClient::new("http://storage.example.com", "ak", "sk")
        .expect("remote client should build");

    let url = client
        .ingress_profile_url(" a/b ")
        .expect("profile URL should build");

    assert_eq!(
        url.path(),
        "/api/v1/internal/storage/ingress-profiles/a%2Fb"
    );
}

#[test]
fn not_found_record_error_uses_contextual_remote_message() {
    let body = serde_json::json!({
        "code": ErrorCode::NotFound as i32,
        "msg": "managed_ingress_profile 'profile-a'",
    })
    .to_string();
    let err = build_remote_status_error_from_parts(
        reqwest::StatusCode::NOT_FOUND,
        &body,
        "update remote ingress profile",
        false,
    );

    assert!(matches!(err, AsterError::RecordNotFound(_)));
    assert_eq!(
        err.message(),
        "update remote ingress profile: managed_ingress_profile 'profile-a'"
    );
}
