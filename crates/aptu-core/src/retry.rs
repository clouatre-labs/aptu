// SPDX-License-Identifier: Apache-2.0

//! Retry logic with exponential backoff for transient failures.
//!
//! Provides helpers to detect retryable errors and configure exponential backoff
//! with jitter for HTTP requests and other transient operations.

use backon::ExponentialBuilder;

/// Determines if an HTTP status code is retryable.
///
/// Retryable status codes are:
/// - 429 (Too Many Requests / Rate Limited)
/// - 500 (Internal Server Error)
/// - 502 (Bad Gateway)
/// - 503 (Service Unavailable)
/// - 504 (Gateway Timeout)
///
/// # Arguments
///
/// * `status` - HTTP status code as u16
///
/// # Returns
///
/// `true` if the status code indicates a transient error that should be retried
#[must_use]
pub fn is_retryable_http(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Determines if an anyhow error is retryable.
///
/// Checks if the error chain contains a retryable HTTP status code or network error.
///
/// # Arguments
///
/// * `e` - Reference to an anyhow error
///
/// # Returns
///
/// `true` if the error is transient and should be retried
#[must_use]
pub fn is_retryable_anyhow(e: &anyhow::Error) -> bool {
    // Check if it's a reqwest error
    if let Some(req_err) = e.downcast_ref::<reqwest::Error>() {
        // Retryable network errors
        if req_err.is_timeout() || req_err.is_connect() {
            return true;
        }
        // Check status code if available
        if let Some(status) = req_err.status() {
            return is_retryable_http(status.as_u16());
        }
    }

    // Check if it's our AptuError with RateLimited
    if let Some(aptu_err) = e.downcast_ref::<crate::error::AptuError>()
        && matches!(aptu_err, crate::error::AptuError::RateLimited { .. })
    {
        return true;
    }

    false
}

/// Creates a configured exponential backoff builder for retries.
///
/// Configuration per SPEC.md:
/// - Factor: 2 (exponential growth)
/// - Min delay: 1 second
/// - Max times: 3 (total of 3 attempts)
/// - Jitter: enabled
///
/// # Returns
///
/// An `ExponentialBuilder` configured for retry operations
#[must_use]
pub fn retry_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_factor(2.0)
        .with_min_delay(std::time::Duration::from_secs(1))
        .with_max_times(3)
        .with_jitter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_http_429() {
        assert!(is_retryable_http(429));
    }

    #[test]
    fn test_is_retryable_http_500() {
        assert!(is_retryable_http(500));
    }

    #[test]
    fn test_is_retryable_http_502() {
        assert!(is_retryable_http(502));
    }

    #[test]
    fn test_is_retryable_http_503() {
        assert!(is_retryable_http(503));
    }

    #[test]
    fn test_is_retryable_http_504() {
        assert!(is_retryable_http(504));
    }

    #[test]
    fn test_is_retryable_http_non_retryable() {
        assert!(!is_retryable_http(400));
        assert!(!is_retryable_http(401));
        assert!(!is_retryable_http(403));
        assert!(!is_retryable_http(404));
        assert!(!is_retryable_http(200));
        assert!(!is_retryable_http(201));
    }

    #[test]
    fn test_retry_backoff_configuration() {
        let backoff = retry_backoff();
        // Verify it's an ExponentialBuilder (type check at compile time)
        let _: ExponentialBuilder = backoff;
    }

    #[test]
    fn test_is_retryable_anyhow_with_non_retryable() {
        let err = anyhow::anyhow!("some other error");
        assert!(!is_retryable_anyhow(&err));
    }
}
