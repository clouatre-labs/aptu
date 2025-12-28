// SPDX-License-Identifier: Apache-2.0

//! Text utility functions for Aptu.
//!
//! Provides reusable text formatting utilities for truncation and relative time display.
//! These functions are used by CLI, and will be available to iOS and MCP consumers.

use chrono::{DateTime, Utc};
use regex::Regex;
use std::process::Command;

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

/// Check if a label is a priority label (p0-p4 or priority: high/medium/low).
///
/// Recognizes two priority label patterns:
/// - Numeric: `p0`, `p1`, `p2`, `p3`, `p4` (case-insensitive)
/// - Named: `priority: high`, `priority: medium`, `priority: low` (case-insensitive)
///
/// # Examples
///
/// ```
/// use aptu_core::utils::is_priority_label;
///
/// // Numeric priority labels
/// assert!(is_priority_label("p0"));
/// assert!(is_priority_label("P3"));
/// assert!(is_priority_label("p4"));
///
/// // Named priority labels
/// assert!(is_priority_label("priority: high"));
/// assert!(is_priority_label("Priority: Medium"));
/// assert!(is_priority_label("PRIORITY: LOW"));
///
/// // Non-priority labels
/// assert!(!is_priority_label("bug"));
/// assert!(!is_priority_label("enhancement"));
/// assert!(!is_priority_label("priority: urgent"));
/// ```
#[must_use]
pub fn is_priority_label(label: &str) -> bool {
    let lower = label.to_lowercase();

    // Check for p[0-9] pattern (e.g., p0, p1, p2, p3, p4)
    if lower.len() == 2
        && lower.starts_with('p')
        && lower.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // Check for priority: prefix (e.g., priority: high, priority: medium, priority: low)
    if lower.starts_with("priority:") {
        let suffix = lower.strip_prefix("priority:").unwrap_or("").trim();
        return matches!(suffix, "high" | "medium" | "low");
    }

    false
}

/// Parses a git remote URL to extract owner/repo.
///
/// Supports SSH (git@github.com:owner/repo.git) and HTTPS
/// (<https://github.com/owner/repo.git>) formats.
///
/// # Examples
///
/// ```
/// use aptu_core::utils::parse_git_remote_url;
///
/// assert_eq!(parse_git_remote_url("git@github.com:owner/repo.git"), Ok("owner/repo".to_string()));
/// assert_eq!(parse_git_remote_url("https://github.com/owner/repo.git"), Ok("owner/repo".to_string()));
/// assert_eq!(parse_git_remote_url("git@github.com:owner/repo"), Ok("owner/repo".to_string()));
/// ```
pub fn parse_git_remote_url(url: &str) -> Result<String, String> {
    // Parse SSH format: git@github.com:owner/repo.git
    if let Some(ssh_part) = url.strip_prefix("git@github.com:") {
        let repo = ssh_part.strip_suffix(".git").unwrap_or(ssh_part);
        return Ok(repo.to_string());
    }

    // Parse HTTPS format: https://github.com/owner/repo.git
    if let Some(https_part) = url.strip_prefix("https://github.com/") {
        let repo = https_part.strip_suffix(".git").unwrap_or(https_part);
        return Ok(repo.to_string());
    }

    // Try generic regex pattern for other git hosts
    let re = Regex::new(r"(?:git@|https://)[^/]+[:/]([^/]+)/(.+?)(?:\.git)?$")
        .map_err(|e| format!("Regex error: {e}"))?;

    if let Some(caps) = re.captures(url)
        && let (Some(owner), Some(repo)) = (caps.get(1), caps.get(2))
    {
        return Ok(format!("{}/{}", owner.as_str(), repo.as_str()));
    }

    Err(format!("Could not parse git remote URL: {url}"))
}

/// Infers the GitHub repository (owner/repo) from the local git config.
///
/// Runs `git config --get remote.origin.url` and parses the result.
///
/// # Errors
///
/// Returns an error if:
/// - Not in a git repository
/// - No origin remote is configured
/// - Origin URL cannot be parsed
///
/// # Examples
///
/// ```no_run
/// use aptu_core::utils::infer_repo_from_git;
///
/// match infer_repo_from_git() {
///     Ok(repo) => println!("Found repo: {}", repo),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn infer_repo_from_git() -> Result<String, String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .map_err(|e| format!("Failed to run git command: {e}"))?;

    if !output.status.success() {
        return Err("Not in a git repository or no origin remote configured".to_string());
    }

    let url = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in git output: {e}"))?
        .trim()
        .to_string();

    if url.is_empty() {
        return Err("No origin remote configured".to_string());
    }

    parse_git_remote_url(&url)
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

    // ========================================================================
    // is_priority_label() tests
    // ========================================================================

    #[test]
    fn is_priority_label_numeric_lowercase() {
        assert!(is_priority_label("p0"));
        assert!(is_priority_label("p1"));
        assert!(is_priority_label("p2"));
        assert!(is_priority_label("p3"));
        assert!(is_priority_label("p4"));
    }

    #[test]
    fn is_priority_label_numeric_uppercase() {
        assert!(is_priority_label("P0"));
        assert!(is_priority_label("P1"));
        assert!(is_priority_label("P2"));
        assert!(is_priority_label("P3"));
        assert!(is_priority_label("P4"));
    }

    #[test]
    fn is_priority_label_named_high() {
        assert!(is_priority_label("priority: high"));
        assert!(is_priority_label("Priority: High"));
        assert!(is_priority_label("PRIORITY: HIGH"));
    }

    #[test]
    fn is_priority_label_named_medium() {
        assert!(is_priority_label("priority: medium"));
        assert!(is_priority_label("Priority: Medium"));
        assert!(is_priority_label("PRIORITY: MEDIUM"));
    }

    #[test]
    fn is_priority_label_named_low() {
        assert!(is_priority_label("priority: low"));
        assert!(is_priority_label("Priority: Low"));
        assert!(is_priority_label("PRIORITY: LOW"));
    }

    #[test]
    fn is_priority_label_named_with_extra_spaces() {
        assert!(is_priority_label("priority:  high"));
        assert!(is_priority_label("priority: high  "));
        assert!(is_priority_label("priority:   medium   "));
    }

    #[test]
    fn is_priority_label_not_priority_invalid_numeric() {
        assert!(!is_priority_label("p"));
        assert!(!is_priority_label("p10"));
        assert!(!is_priority_label("pa"));
        assert!(!is_priority_label("p-1"));
    }

    #[test]
    fn is_priority_label_not_priority_invalid_named() {
        assert!(!is_priority_label("priority: urgent"));
        assert!(!is_priority_label("priority: critical"));
        assert!(!is_priority_label("priority:"));
        assert!(!is_priority_label("priority: "));
    }

    #[test]
    fn is_priority_label_not_priority_other_labels() {
        assert!(!is_priority_label("bug"));
        assert!(!is_priority_label("enhancement"));
        assert!(!is_priority_label("documentation"));
        assert!(!is_priority_label("help wanted"));
        assert!(!is_priority_label("good first issue"));
    }

    // ========================================================================
    // parse_git_remote_url() tests
    // ========================================================================

    #[test]
    fn parse_git_remote_url_ssh_with_git_suffix() {
        assert_eq!(
            parse_git_remote_url("git@github.com:owner/repo.git"),
            Ok("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_git_remote_url_ssh_without_git_suffix() {
        assert_eq!(
            parse_git_remote_url("git@github.com:owner/repo"),
            Ok("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_git_remote_url_https_with_git_suffix() {
        assert_eq!(
            parse_git_remote_url("https://github.com/owner/repo.git"),
            Ok("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_git_remote_url_https_without_git_suffix() {
        assert_eq!(
            parse_git_remote_url("https://github.com/owner/repo"),
            Ok("owner/repo".to_string())
        );
    }

    #[test]
    fn parse_git_remote_url_invalid() {
        assert!(parse_git_remote_url("not-a-url").is_err());
    }
}
