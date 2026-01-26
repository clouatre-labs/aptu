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

/// Determines if an octocrab error is retryable.
///
/// Retryable octocrab errors include:
/// - GitHub API errors with retryable status codes (429, 500, 502, 503, 504, 403)
/// - Service errors (transient)
/// - Hyper errors (network-related)
///
/// # Arguments
///
/// * `e` - Reference to an octocrab error
///
/// # Returns
///
/// `true` if the error is transient and should be retried
#[must_use]
pub fn is_retryable_octocrab(e: &octocrab::Error) -> bool {
    match e {
        octocrab::Error::GitHub { source, .. } => {
            // Check if the GitHub error has a retryable status code
            // 403 is included for GitHub secondary rate limits
            matches!(
                source.status_code.as_u16(),
                429 | 500 | 502 | 503 | 504 | 403
            )
        }
        octocrab::Error::Service { .. } | octocrab::Error::Hyper { .. } => true,
        _ => false,
    }
}

/// Determines if an anyhow error is retryable.
///
/// Checks if the error chain contains a retryable HTTP status code or network error.
/// Supports reqwest, octocrab, and `AptuError` variants.
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
    // Check if it's an octocrab error
    if let Some(oct_err) = e.downcast_ref::<octocrab::Error>() {
        return is_retryable_octocrab(oct_err);
    }

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

    // Check if it's our AptuError with RateLimited or TruncatedResponse
    if let Some(aptu_err) = e.downcast_ref::<crate::error::AptuError>() {
        return matches!(
            aptu_err,
            crate::error::AptuError::RateLimited { .. }
                | crate::error::AptuError::TruncatedResponse { .. }
        );
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

/// Maximum retry-after delay to prevent excessive waits (120 seconds).
const MAX_RETRY_AFTER_SECS: u64 = 120;

/// Extracts `retry_after` value from a `RateLimited` error if present.
///
/// Checks the top-level error for an `AptuError::RateLimited` variant and returns
/// its `retry_after` value. Caps the value at `MAX_RETRY_AFTER_SECS` to prevent
/// excessive waits.
///
/// # Arguments
///
/// * `e` - Reference to an anyhow error
///
/// # Returns
///
/// `Some(duration)` if a `RateLimited` error is found with `retry_after` > 0,
/// `None` otherwise
#[must_use]
pub fn extract_retry_after(e: &anyhow::Error) -> Option<std::time::Duration> {
    if let Some(crate::error::AptuError::RateLimited { retry_after, .. }) =
        e.downcast_ref::<crate::error::AptuError>()
        && *retry_after > 0
    {
        let capped = (*retry_after).min(MAX_RETRY_AFTER_SECS);
        return Some(std::time::Duration::from_secs(capped));
    }
    None
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

    #[test]
    fn test_is_retryable_http_retryable_codes() {
        assert!(is_retryable_http(429));
        assert!(is_retryable_http(500));
        assert!(is_retryable_http(502));
        assert!(is_retryable_http(503));
        assert!(is_retryable_http(504));
    }

    #[test]
    fn test_is_retryable_http_non_retryable_codes() {
        assert!(!is_retryable_http(400));
        assert!(!is_retryable_http(401));
        assert!(!is_retryable_http(403));
        assert!(!is_retryable_http(404));
        assert!(!is_retryable_http(200));
        assert!(!is_retryable_http(201));
    }

    #[test]
    fn test_is_retryable_anyhow_with_truncated_response() {
        let err = anyhow::anyhow!(crate::error::AptuError::TruncatedResponse {
            provider: "OpenRouter".to_string(),
        });
        assert!(is_retryable_anyhow(&err));
    }

    #[test]
    fn test_is_retryable_anyhow_with_rate_limited() {
        let err = anyhow::anyhow!(crate::error::AptuError::RateLimited {
            provider: "OpenRouter".to_string(),
            retry_after: 60,
        });
        assert!(is_retryable_anyhow(&err));
    }

    #[test]
    fn test_extract_retry_after_with_valid_value() {
        let err = anyhow::anyhow!(crate::error::AptuError::RateLimited {
            provider: "OpenRouter".to_string(),
            retry_after: 60,
        });
        let duration = extract_retry_after(&err);
        assert_eq!(duration, Some(std::time::Duration::from_secs(60)));
    }

    #[test]
    fn test_extract_retry_after_with_zero_value() {
        let err = anyhow::anyhow!(crate::error::AptuError::RateLimited {
            provider: "OpenRouter".to_string(),
            retry_after: 0,
        });
        let duration = extract_retry_after(&err);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_extract_retry_after_with_capped_value() {
        let err = anyhow::anyhow!(crate::error::AptuError::RateLimited {
            provider: "OpenRouter".to_string(),
            retry_after: 300,
        });
        let duration = extract_retry_after(&err);
        assert_eq!(duration, Some(std::time::Duration::from_secs(120)));
    }

    #[test]
    fn test_extract_retry_after_with_non_rate_limited_error() {
        let err = anyhow::anyhow!("some other error");
        let duration = extract_retry_after(&err);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_extract_retry_after_with_truncated_response() {
        let err = anyhow::anyhow!(crate::error::AptuError::TruncatedResponse {
            provider: "OpenRouter".to_string(),
        });
        let duration = extract_retry_after(&err);
        assert_eq!(duration, None);
    }
}
