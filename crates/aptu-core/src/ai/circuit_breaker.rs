// SPDX-License-Identifier: Apache-2.0

//! Circuit breaker pattern for AI provider resilience.
//!
//! Protects against sustained failures by tracking consecutive failures
//! and transitioning between Closed, Open, and Half-Open states.
//! Uses `std::sync::atomic` for thread-safe state management without
//! external dependencies.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Circuit breaker state machine for AI provider resilience.
///
/// States:
/// - **Closed**: Normal operation, requests pass through.
/// - **Open**: Threshold exceeded, requests fail immediately.
/// - **Half-Open**: Testing if provider recovered, allows one request.
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Consecutive failure count.
    failure_count: AtomicU32,
    /// Timestamp of last failure (seconds since `UNIX_EPOCH`).
    last_failure_time: AtomicU64,
    /// Failure threshold before opening.
    threshold: u32,
    /// Reset timeout in seconds.
    reset_seconds: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Number of consecutive failures before opening (default: 3).
    /// * `reset_seconds` - Seconds to wait before attempting recovery (default: 60).
    #[must_use]
    pub fn new(threshold: u32, reset_seconds: u64) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            threshold,
            reset_seconds,
        }
    }

    /// Check if the circuit is open (provider unavailable).
    #[must_use]
    pub fn is_open(&self) -> bool {
        let failures = self.failure_count.load(Ordering::Relaxed);

        if failures < self.threshold {
            return false;
        }

        let last_failure = self.last_failure_time.load(Ordering::Relaxed);
        let now = current_time_secs();

        // Still in open state if reset timeout hasn't elapsed
        now < last_failure + self.reset_seconds
    }

    /// Record a successful request (reset failure count).
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }

    /// Record a failed request (increment failure count).
    pub fn record_failure(&self) {
        let new_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Update last failure time when threshold is reached
        if new_count >= self.threshold {
            let now = current_time_secs();
            self.last_failure_time.store(now, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
impl CircuitBreaker {
    /// Get current failure count (for testing/observability).
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Get last failure timestamp (for testing/observability).
    #[must_use]
    pub fn last_failure_time(&self) -> u64 {
        self.last_failure_time.load(Ordering::Relaxed)
    }
}

/// Get current time in seconds since `UNIX_EPOCH`.
fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_circuit_closed_initially() {
        let cb = CircuitBreaker::new(3, 60);
        assert!(!cb.is_open());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 60);

        cb.record_failure();
        assert!(!cb.is_open());

        cb.record_failure();
        assert!(!cb.is_open());

        cb.record_failure();
        assert!(cb.is_open());
        assert_eq!(cb.failure_count(), 3);
    }

    #[test]
    fn test_circuit_closes_on_success() {
        let cb = CircuitBreaker::new(3, 60);

        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        cb.record_success();
        assert!(!cb.is_open());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_half_open_after_reset_timeout() {
        let cb = CircuitBreaker::new(2, 1);

        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        // Wait for reset timeout
        thread::sleep(Duration::from_secs(2));

        // Circuit should be half-open (not open anymore)
        assert!(!cb.is_open());
    }

    #[test]
    fn test_circuit_reopens_on_failure_in_half_open() {
        let cb = CircuitBreaker::new(2, 1);

        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        thread::sleep(Duration::from_secs(2));
        assert!(!cb.is_open());

        // Failure in half-open state
        cb.record_failure();
        assert!(cb.is_open());
    }

    #[test]
    fn test_custom_threshold_and_reset() {
        let cb = CircuitBreaker::new(5, 120);

        for _ in 0..4 {
            cb.record_failure();
        }
        assert!(!cb.is_open());

        cb.record_failure();
        assert!(cb.is_open());
    }
}
