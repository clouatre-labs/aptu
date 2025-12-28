// SPDX-License-Identifier: Apache-2.0

//! GitHub integration module.
//!
//! Provides authentication and API client functionality for GitHub.

use anyhow::{Context, Result};
use tracing::debug;

pub mod auth;
pub mod graphql;
pub mod issues;
pub mod pulls;
pub mod ratelimit;
pub mod releases;

/// OAuth Client ID for Aptu CLI (safe to embed per RFC 8252).
///
/// This is a public client ID for native/CLI applications. Per OAuth 2.0 for
/// Native Apps (RFC 8252), client credentials in native apps cannot be kept
/// confidential and are safe to embed in source code.
pub const OAUTH_CLIENT_ID: &str = "Ov23lifiYQrh6Ga7Hpyr";

/// Keyring service name for storing credentials.
#[cfg(feature = "keyring")]
pub const KEYRING_SERVICE: &str = "aptu";

/// Keyring username for the GitHub token.
#[cfg(feature = "keyring")]
pub const KEYRING_USER: &str = "github_token";

/// Discriminator for GitHub reference type (issue or pull request).
#[derive(Debug, Clone, Copy)]
pub enum ReferenceKind {
    /// Issue reference with display name and URL path segment.
    Issue,
    /// Pull request reference with display name and URL path segment.
    Pull,
}

impl ReferenceKind {
    /// Returns the display name for this reference kind.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            ReferenceKind::Issue => "issue",
            ReferenceKind::Pull => "pull request",
        }
    }

    /// Returns the URL path segment for this reference kind.
    #[must_use]
    pub fn url_segment(&self) -> &'static str {
        match self {
            ReferenceKind::Issue => "issues",
            ReferenceKind::Pull => "pull",
        }
    }
}

/// Parses an owner/repo string to extract owner and repo.
///
/// Validates format: exactly one `/`, non-empty parts.
///
/// # Errors
///
/// Returns an error if the format is invalid.
pub fn parse_owner_repo(s: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        anyhow::bail!(
            "Invalid owner/repo format.\n\
             Expected: owner/repo\n\
             Got: {s}"
        );
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Parses a GitHub reference (issue or PR) in multiple formats.
///
/// Supports:
/// - Full URL: `https://github.com/owner/repo/issues/123` or `https://github.com/owner/repo/pull/123`
/// - Short form: `owner/repo#123`
/// - Bare number: `123` (requires `repo_context`)
///
/// # Arguments
///
/// * `kind` - The type of reference (Issue or Pull)
/// * `input` - The reference to parse
/// * `repo_context` - Optional repository context for bare numbers (e.g., "owner/repo")
///
/// # Errors
///
/// Returns an error if the format is invalid or bare number is used without context.
pub fn parse_github_reference(
    kind: ReferenceKind,
    input: &str,
    repo_context: Option<&str>,
) -> Result<(String, String, u64)> {
    let input = input.trim();

    // Try full URL first
    if input.starts_with("https://") || input.starts_with("http://") {
        // Remove trailing fragments and query params
        let clean_url = input.split('#').next().unwrap_or(input);
        let clean_url = clean_url.split('?').next().unwrap_or(clean_url);

        // Parse the URL path
        let parts: Vec<&str> = clean_url.trim_end_matches('/').split('/').collect();

        // Expected: ["https:", "", "github.com", "owner", "repo", "issues/pull", "123"]
        if parts.len() < 7 {
            anyhow::bail!(
                "Invalid GitHub {} URL format.\n\
                 Expected: https://github.com/owner/repo/{}/123\n\
                 Got: {input}",
                kind.display_name(),
                kind.url_segment()
            );
        }

        // Verify it's a github.com URL
        if !parts[2].contains("github.com") {
            anyhow::bail!(
                "URL must be a GitHub {} URL.\n\
                 Expected: https://github.com/owner/repo/{}/123\n\
                 Got: {input}",
                kind.display_name(),
                kind.url_segment()
            );
        }

        // Verify it's the correct path segment
        if parts[5] != kind.url_segment() {
            anyhow::bail!(
                "URL must point to a GitHub {}.\n\
                 Expected: https://github.com/owner/repo/{}/123\n\
                 Got: {input}",
                kind.display_name(),
                kind.url_segment()
            );
        }

        let owner = parts[3].to_string();
        let repo = parts[4].to_string();
        let number: u64 = parts[6].parse().with_context(|| {
            format!(
                "Invalid {} number '{}' in URL.\n\
                 Expected a numeric {} number.",
                kind.display_name(),
                parts[6],
                kind.display_name()
            )
        })?;

        debug!(owner = %owner, repo = %repo, number = number, "Parsed {} URL", kind.display_name());
        return Ok((owner, repo, number));
    }

    // Try short form: owner/repo#123
    if let Some(hash_pos) = input.find('#') {
        let owner_repo_part = &input[..hash_pos];
        let number_part = &input[hash_pos + 1..];

        let (owner, repo) = parse_owner_repo(owner_repo_part)?;
        let number: u64 = number_part.parse().with_context(|| {
            format!(
                "Invalid {} number '{number_part}' in short form.\n\
                 Expected: owner/repo#123\n\
                 Got: {input}",
                kind.display_name()
            )
        })?;

        debug!(owner = %owner, repo = %repo, number = number, "Parsed short-form {} reference", kind.display_name());
        return Ok((owner, repo, number));
    }

    // Try bare number: 123 (requires repo_context)
    if let Ok(number) = input.parse::<u64>() {
        let repo_context = repo_context.ok_or_else(|| {
            anyhow::anyhow!(
                "Bare {} number requires repository context.\n\
                 Use one of:\n\
                 - Full URL: https://github.com/owner/repo/{}/123\n\
                 - Short form: owner/repo#123\n\
                 - Bare number with --repo flag: 123 --repo owner/repo\n\
                 Got: {input}",
                kind.display_name(),
                kind.url_segment()
            )
        })?;

        let (owner, repo) = parse_owner_repo(repo_context)?;
        debug!(owner = %owner, repo = %repo, number = number, "Parsed bare {} number", kind.display_name());
        return Ok((owner, repo, number));
    }

    // If we get here, it's an invalid format
    anyhow::bail!(
        "Invalid {} reference format.\n\
         Expected one of:\n\
         - Full URL: https://github.com/owner/repo/{}/123\n\
         - Short form: owner/repo#123\n\
         - Bare number with --repo flag: 123 --repo owner/repo\n\
         Got: {input}",
        kind.display_name(),
        kind.url_segment()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owner_repo_valid() {
        let (owner, repo) = parse_owner_repo("octocat/Hello-World").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
    }

    #[test]
    fn test_parse_owner_repo_invalid_no_slash() {
        assert!(parse_owner_repo("octocat").is_err());
    }

    #[test]
    fn test_parse_owner_repo_invalid_empty_owner() {
        assert!(parse_owner_repo("/repo").is_err());
    }

    #[test]
    fn test_parse_owner_repo_invalid_empty_repo() {
        assert!(parse_owner_repo("owner/").is_err());
    }

    #[test]
    fn test_parse_github_reference_issue_full_url() {
        let (owner, repo, number) = parse_github_reference(
            ReferenceKind::Issue,
            "https://github.com/octocat/Hello-World/issues/123",
            None,
        )
        .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_github_reference_issue_full_url_with_query() {
        let (owner, repo, number) = parse_github_reference(
            ReferenceKind::Issue,
            "https://github.com/octocat/Hello-World/issues/123?foo=bar",
            None,
        )
        .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_github_reference_issue_full_url_with_fragment() {
        let (owner, repo, number) = parse_github_reference(
            ReferenceKind::Issue,
            "https://github.com/octocat/Hello-World/issues/123#comment-456",
            None,
        )
        .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_github_reference_issue_short_form() {
        let (owner, repo, number) =
            parse_github_reference(ReferenceKind::Issue, "octocat/Hello-World#123", None).unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_github_reference_issue_bare_number() {
        let (owner, repo, number) =
            parse_github_reference(ReferenceKind::Issue, "123", Some("octocat/Hello-World"))
                .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_github_reference_issue_bare_number_no_context() {
        assert!(parse_github_reference(ReferenceKind::Issue, "123", None).is_err());
    }

    #[test]
    fn test_parse_github_reference_pull_full_url() {
        let (owner, repo, number) = parse_github_reference(
            ReferenceKind::Pull,
            "https://github.com/octocat/Hello-World/pull/456",
            None,
        )
        .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 456);
    }

    #[test]
    fn test_parse_github_reference_pull_short_form() {
        let (owner, repo, number) =
            parse_github_reference(ReferenceKind::Pull, "octocat/Hello-World#456", None).unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 456);
    }

    #[test]
    fn test_parse_github_reference_pull_bare_number() {
        let (owner, repo, number) =
            parse_github_reference(ReferenceKind::Pull, "456", Some("octocat/Hello-World"))
                .unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
        assert_eq!(number, 456);
    }

    #[test]
    fn test_parse_github_reference_issue_wrong_kind_url() {
        // Try to parse a PR URL as an issue
        assert!(
            parse_github_reference(
                ReferenceKind::Issue,
                "https://github.com/octocat/Hello-World/pull/123",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_parse_github_reference_pull_wrong_kind_url() {
        // Try to parse an issue URL as a PR
        assert!(
            parse_github_reference(
                ReferenceKind::Pull,
                "https://github.com/octocat/Hello-World/issues/123",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_parse_github_reference_invalid_url() {
        assert!(
            parse_github_reference(
                ReferenceKind::Issue,
                "https://github.com/octocat/Hello-World/invalid/123",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_parse_github_reference_not_github_url() {
        assert!(
            parse_github_reference(
                ReferenceKind::Issue,
                "https://gitlab.com/octocat/Hello-World/issues/123",
                None
            )
            .is_err()
        );
    }
}
