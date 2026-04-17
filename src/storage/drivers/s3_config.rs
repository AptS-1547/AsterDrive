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
    let mut bucket = bucket.trim().to_string();

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

    let authority = uri.authority().ok_or_else(|| {
        AsterError::validation_error(format!("S3 endpoint must include a hostname: '{endpoint}'"))
    })?;
    let host = authority.host();

    if is_r2_public_host(host) {
        return Err(AsterError::validation_error(
            "Cloudflare R2 endpoint must use the account-level S3 API host (https://<account>.r2.cloudflarestorage.com), not a public r2.dev URL",
        ));
    }

    if !is_r2_api_host(host) {
        if bucket.is_empty() {
            return Err(AsterError::validation_error(
                "bucket is required for S3-compatible storage",
            ));
        }

        return Ok(NormalizedS3Config {
            endpoint: endpoint.to_string(),
            bucket,
        });
    }

    if uri
        .path_and_query()
        .and_then(|value| value.query())
        .is_some()
    {
        return Err(AsterError::validation_error(
            "Cloudflare R2 endpoint must not include query parameters",
        ));
    }

    let path = uri.path().trim_matches('/');
    if !path.is_empty() {
        if path.contains('/') {
            return Err(AsterError::validation_error(
                "Cloudflare R2 endpoint must use the account-level endpoint root; put only the bucket name in the bucket field",
            ));
        }

        if bucket.is_empty() {
            bucket = path.to_string();
        } else if bucket != path {
            return Err(AsterError::validation_error(format!(
                "Cloudflare R2 endpoint bucket '{path}' does not match bucket field '{bucket}'"
            )));
        }
    }

    if bucket.is_empty() {
        return Err(AsterError::validation_error(
            "bucket is required for S3-compatible storage",
        ));
    }

    Ok(NormalizedS3Config {
        endpoint: format!("{scheme}://{authority}"),
        bucket,
    })
}

fn is_r2_api_host(host: &str) -> bool {
    host == "r2.cloudflarestorage.com" || host.ends_with(".r2.cloudflarestorage.com")
}

fn is_r2_public_host(host: &str) -> bool {
    host == "r2.dev" || host.ends_with(".r2.dev")
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
    fn extracts_bucket_from_r2_endpoint_path() {
        let normalized = normalize_s3_endpoint_and_bucket(
            "https://demo-account.r2.cloudflarestorage.com/photos",
            "",
        )
        .expect("normalized R2 config");

        assert_eq!(
            normalized.endpoint,
            "https://demo-account.r2.cloudflarestorage.com"
        );
        assert_eq!(normalized.bucket, "photos");
    }

    #[test]
    fn rejects_public_r2_dev_endpoint() {
        let err = normalize_s3_endpoint_and_bucket("https://pub-demo.r2.dev", "photos")
            .expect_err("public R2 host should fail");

        assert_eq!(err.code(), "E005");
        assert!(
            err.message().contains("r2.dev"),
            "expected R2 public host hint in '{}'",
            err.message()
        );
    }

    #[test]
    fn rejects_mismatched_r2_bucket() {
        let err = normalize_s3_endpoint_and_bucket(
            "https://demo-account.r2.cloudflarestorage.com/photos",
            "videos",
        )
        .expect_err("mismatched R2 bucket should fail");

        assert_eq!(err.code(), "E005");
        assert!(
            err.message().contains("does not match bucket field"),
            "expected bucket mismatch hint in '{}'",
            err.message()
        );
    }

    #[test]
    fn rejects_missing_bucket_when_endpoint_has_no_r2_bucket_path() {
        let err =
            normalize_s3_endpoint_and_bucket("https://demo-account.r2.cloudflarestorage.com", "")
                .expect_err("missing bucket should fail");

        assert_eq!(err.code(), "E005");
        assert!(
            err.message().contains("bucket is required"),
            "expected missing bucket hint in '{}'",
            err.message()
        );
    }
}
