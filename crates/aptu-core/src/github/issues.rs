//! GitHub issue operations for the triage command.
//!
//! Provides functionality to parse issue URLs, fetch issue details,
//! and post triage comments.

use anyhow::{Context, Result};
use octocrab::Octocrab;
use tracing::{debug, instrument};

use crate::ai::types::{IssueComment, IssueDetails};

/// Parses an owner/repo string to extract owner and repo.
///
/// Validates format: exactly one `/`, non-empty parts.
///
/// # Errors
///
/// Returns an error if the format is invalid.
fn parse_owner_repo(s: &str) -> Result<(String, String)> {
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

/// Parses a GitHub issue reference in multiple formats.
///
/// Supports:
/// - Full URL: `https://github.com/owner/repo/issues/123`
/// - Short form: `owner/repo#123`
/// - Bare number: `123` (requires `repo_context`)
///
/// # Arguments
///
/// * `input` - The issue reference to parse
/// * `repo_context` - Optional repository context for bare numbers (e.g., "owner/repo")
///
/// # Errors
///
/// Returns an error if the format is invalid or bare number is used without context.
pub fn parse_issue_reference(
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

        // Expected: ["https:", "", "github.com", "owner", "repo", "issues", "123"]
        if parts.len() < 7 {
            anyhow::bail!(
                "Invalid GitHub issue URL format.\n\
                 Expected: https://github.com/owner/repo/issues/123\n\
                 Got: {input}"
            );
        }

        // Verify it's a github.com URL
        if !parts[2].contains("github.com") {
            anyhow::bail!(
                "URL must be a GitHub issue URL.\n\
                 Expected: https://github.com/owner/repo/issues/123\n\
                 Got: {input}"
            );
        }

        // Verify it's an issues path
        if parts[5] != "issues" {
            anyhow::bail!(
                "URL must point to a GitHub issue.\n\
                 Expected: https://github.com/owner/repo/issues/123\n\
                 Got: {input}"
            );
        }

        let owner = parts[3].to_string();
        let repo = parts[4].to_string();
        let number: u64 = parts[6].parse().with_context(|| {
            format!(
                "Invalid issue number '{}' in URL.\n\
                 Expected a numeric issue number.",
                parts[6]
            )
        })?;

        debug!(owner = %owner, repo = %repo, number = number, "Parsed issue URL");
        return Ok((owner, repo, number));
    }

    // Try short form: owner/repo#123
    if let Some(hash_pos) = input.find('#') {
        let owner_repo_part = &input[..hash_pos];
        let number_part = &input[hash_pos + 1..];

        let (owner, repo) = parse_owner_repo(owner_repo_part)?;
        let number: u64 = number_part.parse().with_context(|| {
            format!(
                "Invalid issue number '{number_part}' in short form.\n\
                 Expected: owner/repo#123\n\
                 Got: {input}"
            )
        })?;

        debug!(owner = %owner, repo = %repo, number = number, "Parsed short-form issue reference");
        return Ok((owner, repo, number));
    }

    // Try bare number: 123 (requires repo_context)
    if let Ok(number) = input.parse::<u64>() {
        let repo_context = repo_context.ok_or_else(|| {
            anyhow::anyhow!(
                "Bare issue number requires repository context.\n\
                 Use one of:\n\
                 - Full URL: https://github.com/owner/repo/issues/123\n\
                 - Short form: owner/repo#123\n\
                 - Bare number with --repo flag: 123 --repo owner/repo\n\
                 Got: {input}"
            )
        })?;

        let (owner, repo) = parse_owner_repo(repo_context)?;
        debug!(owner = %owner, repo = %repo, number = number, "Parsed bare issue number");
        return Ok((owner, repo, number));
    }

    // If we get here, it's an invalid format
    anyhow::bail!(
        "Invalid issue reference format.\n\
         Expected one of:\n\
         - Full URL: https://github.com/owner/repo/issues/123\n\
         - Short form: owner/repo#123\n\
         - Bare number with --repo flag: 123 --repo owner/repo\n\
         Got: {input}"
    );
}

/// Fetches issue details including comments from GitHub.
///
/// # Errors
///
/// Returns an error if the API request fails or the issue is not found.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
pub async fn fetch_issue_with_comments(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<IssueDetails> {
    debug!("Fetching issue details");

    // Fetch the issue
    let issue = client
        .issues(owner, repo)
        .get(number)
        .await
        .with_context(|| format!("Failed to fetch issue #{number} from {owner}/{repo}"))?;

    // Fetch comments (limited to first page)
    let comments_page = client
        .issues(owner, repo)
        .list_comments(number)
        .per_page(5)
        .send()
        .await
        .with_context(|| format!("Failed to fetch comments for issue #{number}"))?;

    // Convert to our types
    let labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();

    let comments: Vec<IssueComment> = comments_page
        .items
        .iter()
        .map(|c| IssueComment {
            author: c.user.login.clone(),
            body: c.body.clone().unwrap_or_default(),
        })
        .collect();

    let issue_url = issue.html_url.to_string();

    let details = IssueDetails {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        title: issue.title,
        body: issue.body.unwrap_or_default(),
        labels,
        comments,
        url: issue_url,
    };

    debug!(
        labels = details.labels.len(),
        comments = details.comments.len(),
        "Fetched issue details"
    );

    Ok(details)
}

/// Posts a triage comment to a GitHub issue.
///
/// # Returns
///
/// The URL of the created comment.
///
/// # Errors
///
/// Returns an error if the API request fails.
#[instrument(skip(client, body), fields(owner = %owner, repo = %repo, number = number))]
pub async fn post_comment(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    body: &str,
) -> Result<String> {
    debug!("Posting triage comment");

    let comment = client
        .issues(owner, repo)
        .create_comment(number, body)
        .await
        .with_context(|| format!("Failed to post comment to issue #{number}"))?;

    let comment_url = comment.html_url.to_string();

    debug!(url = %comment_url, "Comment posted successfully");

    Ok(comment_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reference_full_url() {
        let url = "https://github.com/block/goose/issues/5836";
        let (owner, repo, number) = parse_issue_reference(url, None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn parse_reference_short_form() {
        let reference = "block/goose#5836";
        let (owner, repo, number) = parse_issue_reference(reference, None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn parse_reference_short_form_with_context() {
        let reference = "block/goose#5836";
        let (owner, repo, number) =
            parse_issue_reference(reference, Some("astral-sh/ruff")).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn parse_reference_bare_number_with_context() {
        let reference = "5836";
        let (owner, repo, number) = parse_issue_reference(reference, Some("block/goose")).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn parse_reference_bare_number_without_context() {
        let reference = "5836";
        let result = parse_issue_reference(reference, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Bare issue number requires repository context")
        );
    }

    #[test]
    fn parse_reference_invalid_short_form_missing_slash() {
        let reference = "owner#123";
        let result = parse_issue_reference(reference, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid owner/repo format")
        );
    }

    #[test]
    fn parse_reference_invalid_short_form_extra_slash() {
        let reference = "owner/repo/extra#123";
        let result = parse_issue_reference(reference, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid owner/repo format")
        );
    }

    #[test]
    fn parse_reference_invalid_bare_number() {
        let reference = "abc";
        let result = parse_issue_reference(reference, Some("block/goose"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid issue reference format")
        );
    }

    #[test]
    fn parse_reference_whitespace_trimming() {
        let reference = "  block/goose#5836  ";
        let (owner, repo, number) = parse_issue_reference(reference, None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn parse_reference_bare_number_whitespace() {
        let reference = "  5836  ";
        let (owner, repo, number) = parse_issue_reference(reference, Some("block/goose")).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }
}
