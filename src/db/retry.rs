use std::time::Duration;
use tokio::time::sleep;

use crate::errors::{AsterError, Result};

#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
        }
    }
}

/// Execute an async operation with exponential backoff retry
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = None;
    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt == config.max_retries || !is_retryable(&e) {
                    return Err(e);
                }
                let delay = calculate_delay(config, attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    max = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "retrying operation"
                );
                last_err = Some(e);
                sleep(delay).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AsterError::database_operation("retry exhausted")))
}

fn is_retryable(err: &AsterError) -> bool {
    // Database errors are potentially retryable (deadlock, timeout, connection lost)
    matches!(
        err,
        AsterError::DatabaseOperation(_) | AsterError::DatabaseConnection(_)
    )
}

fn calculate_delay(config: &RetryConfig, attempt: u32) -> Duration {
    use rand::RngExt;
    let base = config.base_delay_ms * 2u64.pow(attempt);
    let capped = base.min(config.max_delay_ms);
    // Add jitter: 50%-150% of the delay
    let mut rng = rand::rng();
    let jitter = rng.random_range(50..=150);
    Duration::from_millis(capped * jitter / 100)
}
