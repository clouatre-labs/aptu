// SPDX-License-Identifier: Apache-2.0

//! Text utility functions for Aptu.
//!
//! Provides reusable text formatting utilities for truncation and relative time display.
//! These functions are used by CLI, and will be available to iOS and MCP consumers.

use chrono::{DateTime, Utc};

/// Truncates text to a maximum length with a custom suffix.
///
/// Uses character count (not byte count) to safely handle multi-byte UTF-8.
/// The suffix is included in the max length calculation.
///
/// # Examples
///
/// ```
/// use aptu_core::utils::truncate_with_suffix;
///
/// let text = "This is a very long string that needs truncation";
/// let result = truncate_with_suffix(text, 20, "... [more]");
/// assert!(result.ends_with("... [more]"));
/// assert!(result.chars().count() <= 20);
/// ```
#[must_use]
pub fn truncate_with_suffix(text: &str, max_len: usize, suffix: &str) -> String {
    let char_count = text.chars().count();
    if char_count <= max_len {
        text.to_string()
    } else {
        let suffix_len = suffix.chars().count();
        let truncate_at = max_len.saturating_sub(suffix_len);
        let truncated: String = text.chars().take(truncate_at).collect();
        format!("{truncated}{suffix}")
    }
}

/// Truncates text to a maximum length with default ellipsis suffix "...".
///
/// Uses character count (not byte count) to safely handle multi-byte UTF-8.
///
/// # Examples
///
/// ```
/// use aptu_core::utils::truncate;
///
/// // Short text unchanged
/// assert_eq!(truncate("Hello", 10), "Hello");
///
/// // Long text truncated with ellipsis
/// let long = "This is a very long title that exceeds the limit";
/// let result = truncate(long, 20);
/// assert!(result.ends_with("..."));
/// assert!(result.chars().count() <= 20);
/// ```
#[must_use]
pub fn truncate(text: &str, max_len: usize) -> String {
    truncate_with_suffix(text, max_len, "...")
}

/// Formats a `DateTime<Utc>` as relative time (e.g., "3 days ago").
///
/// # Examples
///
/// ```
/// use chrono::{Utc, Duration};
/// use aptu_core::utils::format_relative_time;
///
/// let now = Utc::now();
/// assert_eq!(format_relative_time(&now), "just now");
///
/// let yesterday = now - Duration::days(1);
/// assert_eq!(format_relative_time(&yesterday), "1 day ago");
/// ```
#[must_use]
pub fn format_relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_days() > 30 {
        let months = duration.num_days() / 30;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        }
    } else if duration.num_days() > 0 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        }
    } else {
        "just now".to_string()
    }
}

/// Parses an ISO 8601 timestamp and formats it as relative time.
///
/// Returns the original string if parsing fails.
///
/// # Examples
///
/// ```
/// use aptu_core::utils::parse_and_format_relative_time;
///
/// // Valid timestamp
/// let result = parse_and_format_relative_time("2024-01-01T00:00:00Z");
/// assert!(result.contains("ago") || result.contains("months"));
///
/// // Invalid timestamp returns original
/// let invalid = parse_and_format_relative_time("not-a-date");
/// assert_eq!(invalid, "not-a-date");
/// ```
#[must_use]
pub fn parse_and_format_relative_time(timestamp: &str) -> String {
    match timestamp.parse::<DateTime<Utc>>() {
        Ok(dt) => format_relative_time(&dt),
        Err(_) => timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // ========================================================================
    // truncate() tests
    // ========================================================================

    #[test]
    fn truncate_short_text_unchanged() {
        assert_eq!(truncate("Short title", 50), "Short title");
    }

    #[test]
    fn truncate_long_text_with_ellipsis() {
        let long =
            "This is a very long title that should be truncated because it exceeds the limit";
        let result = truncate(long, 30);
        assert_eq!(result.chars().count(), 30);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        let text = "Exactly twenty chars";
        assert_eq!(truncate(text, 20), text);
    }

    #[test]
    fn truncate_utf8_multibyte_safe() {
        // Emoji and multibyte characters should be handled correctly
        let title = "Fix emoji handling in parser";
        let result = truncate(title, 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.ends_with("..."));
    }

    // ========================================================================
    // truncate_with_suffix() tests
    // ========================================================================

    #[test]
    fn truncate_with_suffix_short_text_unchanged() {
        let body = "Short body";
        assert_eq!(
            truncate_with_suffix(body, 100, "... [truncated]"),
            "Short body"
        );
    }

    #[test]
    fn truncate_with_suffix_long_text() {
        let body = "This is a very long body that should be truncated because it exceeds the maximum length";
        let result = truncate_with_suffix(body, 50, "... [truncated]");
        assert!(result.ends_with("... [truncated]"));
        assert!(result.chars().count() <= 50);
    }

    #[test]
    fn truncate_with_suffix_exact_length() {
        let body = "Exactly fifty characters long text here now ok ye";
        let result = truncate_with_suffix(body, 50, "... [truncated]");
        // 49 chars, should not be truncated
        assert_eq!(result, body);
    }

    // ========================================================================
    // format_relative_time() tests
    // ========================================================================

    #[test]
    fn relative_time_just_now() {
        let now = Utc::now();
        assert_eq!(format_relative_time(&now), "just now");
    }

    #[test]
    fn relative_time_one_hour() {
        let one_hour_ago = Utc::now() - Duration::hours(1);
        assert_eq!(format_relative_time(&one_hour_ago), "1 hour ago");
    }

    #[test]
    fn relative_time_multiple_hours() {
        let five_hours_ago = Utc::now() - Duration::hours(5);
        assert_eq!(format_relative_time(&five_hours_ago), "5 hours ago");
    }

    #[test]
    fn relative_time_one_day() {
        let one_day_ago = Utc::now() - Duration::days(1);
        assert_eq!(format_relative_time(&one_day_ago), "1 day ago");
    }

    #[test]
    fn relative_time_multiple_days() {
        let three_days_ago = Utc::now() - Duration::days(3);
        assert_eq!(format_relative_time(&three_days_ago), "3 days ago");
    }

    #[test]
    fn relative_time_one_month() {
        let one_month_ago = Utc::now() - Duration::days(31);
        assert_eq!(format_relative_time(&one_month_ago), "1 month ago");
    }

    #[test]
    fn relative_time_multiple_months() {
        let two_months_ago = Utc::now() - Duration::days(65);
        assert_eq!(format_relative_time(&two_months_ago), "2 months ago");
    }

    // ========================================================================
    // parse_and_format_relative_time() tests
    // ========================================================================

    #[test]
    fn parse_valid_timestamp() {
        let three_days_ago = (Utc::now() - Duration::days(3)).to_rfc3339();
        assert_eq!(
            parse_and_format_relative_time(&three_days_ago),
            "3 days ago"
        );
    }

    #[test]
    fn parse_invalid_timestamp_returns_original() {
        let invalid = "not-a-valid-timestamp";
        assert_eq!(parse_and_format_relative_time(invalid), invalid);
    }
}
