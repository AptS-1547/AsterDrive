//! API-wide constants for time durations and cache headers.

/// 1 hour in seconds.
pub const HOUR_SECS: u64 = 60 * 60;
/// 1 year in seconds (365 days).
pub const YEAR_SECS: u64 = 365 * 24 * HOUR_SECS;

/// Shared OpenAPI description for 401 responses.
pub const OPENAPI_UNAUTHORIZED: &str = "Unauthorized";
/// Shared OpenAPI description for 404 responses.
pub const OPENAPI_NOT_FOUND: &str = "Not found";
