// SPDX-License-Identifier: Apache-2.0

//! GitHub API rate limit checking.
//!
//! Provides utilities to check and report on GitHub API rate limit status.

use anyhow::Result;
use tracing::debug;

/// GitHub API rate limit status.
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Number of API calls remaining in the current rate limit window.
    pub remaining: u32,
    /// Total number of API calls allowed in the rate limit window.
    pub limit: u32,
    /// Unix timestamp when the rate limit resets.
    pub reset_at: u64,
}

impl RateLimitStatus {
    /// Returns true if rate limit is low (remaining < 100).
    #[must_use]
    pub fn is_low(&self) -> bool {
        self.remaining < 100
    }

    /// Returns a human-readable status message.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "GitHub API: {}/{} calls remaining",
            self.remaining, self.limit
        )
    }
}

/// Checks the GitHub API rate limit status.
///
/// Uses the authenticated Octocrab client to fetch the current rate limit
/// information from the GitHub API.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
///
/// # Returns
///
/// `RateLimitStatus` with current rate limit information
///
/// # Errors
///
/// Returns an error if the API request fails.
pub async fn check_rate_limit(client: &octocrab::Octocrab) -> Result<RateLimitStatus> {
    debug!("Checking GitHub API rate limit");

    let rate_limit = client.ratelimit().get().await?;

    #[allow(clippy::cast_possible_truncation)]
    let status = RateLimitStatus {
        remaining: rate_limit.resources.core.remaining as u32,
        limit: rate_limit.resources.core.limit as u32,
        reset_at: rate_limit.resources.core.reset,
    };

    debug!(
        remaining = status.remaining,
        limit = status.limit,
        "GitHub rate limit status"
    );

    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_status_is_low_true() {
        let status = RateLimitStatus {
            remaining: 50,
            limit: 5000,
            reset_at: 1_234_567_890,
        };
        assert!(status.is_low());
    }

    #[test]
    fn test_rate_limit_status_is_low_false() {
        let status = RateLimitStatus {
            remaining: 150,
            limit: 5000,
            reset_at: 1_234_567_890,
        };
        assert!(!status.is_low());
    }

    #[test]
    fn test_rate_limit_status_is_low_boundary() {
        let status = RateLimitStatus {
            remaining: 100,
            limit: 5000,
            reset_at: 1_234_567_890,
        };
        assert!(!status.is_low());
    }

    #[test]
    fn test_rate_limit_status_message() {
        let status = RateLimitStatus {
            remaining: 42,
            limit: 5000,
            reset_at: 1_234_567_890,
        };
        assert_eq!(status.message(), "GitHub API: 42/5000 calls remaining");
    }
}
