//! 存储驱动实现：`s3_config`。

use crate::errors::{AsterError, Result};
use http::Uri;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedS3Config {
    pub endpoint: String,
    pub bucket: String,
}

pub fn normalize_s3_endpoint_and_bucket(
    endpoint: &str,
    bucket: &str,
) -> Result<NormalizedS3Config> {
    let endpoint = endpoint.trim();
    let bucket = bucket.trim().to_string();

    if endpoint.is_empty() {
        if bucket.is_empty() {
            return Err(AsterError::validation_error(
                "bucket is required for S3-compatible storage",
            ));
        }

        return Ok(NormalizedS3Config {
            endpoint: String::new(),
            bucket,
        });
    }

    let uri: Uri = endpoint.parse().map_err(|_| {
        AsterError::validation_error(format!("invalid S3 endpoint URL: '{endpoint}'"))
    })?;

    let scheme = uri.scheme_str().ok_or_else(|| {
        AsterError::validation_error(format!(
            "S3 endpoint must include http:// or https://: '{endpoint}'"
        ))
    })?;
    if scheme != "http" && scheme != "https" {
        return Err(AsterError::validation_error(format!(
            "S3 endpoint must use http:// or https://: '{endpoint}'"
        )));
    }

    uri.authority().ok_or_else(|| {
        AsterError::validation_error(format!("S3 endpoint must include a hostname: '{endpoint}'"))
    })?;

    if bucket.is_empty() {
        return Err(AsterError::validation_error(
            "bucket is required for S3-compatible storage",
        ));
    }

    Ok(NormalizedS3Config {
        endpoint: endpoint.to_string(),
        bucket,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_s3_endpoint_and_bucket;

    #[test]
    fn allows_standard_s3_endpoint_without_rewriting() {
        let normalized =
            normalize_s3_endpoint_and_bucket("https://s3.example.com/custom/path", "archive")
                .expect("normalized S3 config");

        assert_eq!(normalized.endpoint, "https://s3.example.com/custom/path");
        assert_eq!(normalized.bucket, "archive");
    }

    #[test]
    fn rejects_missing_bucket_for_any_s3_compatible_endpoint() {
        let err = normalize_s3_endpoint_and_bucket("https://s3.example.com", "")
            .expect_err("missing bucket should fail");

        assert_eq!(err.code(), "E005");
        assert!(
            err.message().contains("bucket is required"),
            "expected missing bucket hint in '{}'",
            err.message()
        );
    }
}
