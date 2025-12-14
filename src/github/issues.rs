//! GitHub issue operations for the triage command.
//!
//! Provides functionality to parse issue URLs, fetch issue details,
//! and post triage comments.

use anyhow::{Context, Result};
use octocrab::Octocrab;
use tracing::{debug, instrument};

use crate::ai::types::{IssueComment, IssueDetails};

/// Parses a GitHub issue URL to extract owner, repo, and issue number.
///
/// Supports URLs in the format:
/// - `https://github.com/owner/repo/issues/123`
/// - `https://github.com/owner/repo/issues/123#issuecomment-456`
///
/// # Errors
///
/// Returns an error if the URL does not match the expected format.
pub fn parse_issue_url(url: &str) -> Result<(String, String, u64)> {
    // Remove trailing fragments and query params
    let clean_url = url.split('#').next().unwrap_or(url);
    let clean_url = clean_url.split('?').next().unwrap_or(clean_url);

    // Parse the URL path
    let parts: Vec<&str> = clean_url.trim_end_matches('/').split('/').collect();

    // Expected: ["https:", "", "github.com", "owner", "repo", "issues", "123"]
    if parts.len() < 7 {
        anyhow::bail!(
            "Invalid GitHub issue URL format.\n\
             Expected: https://github.com/owner/repo/issues/123\n\
             Got: {}",
            url
        );
    }

    // Verify it's a github.com URL
    if !parts[2].contains("github.com") {
        anyhow::bail!(
            "URL must be a GitHub issue URL.\n\
             Expected: https://github.com/owner/repo/issues/123\n\
             Got: {}",
            url
        );
    }

    // Verify it's an issues path
    if parts[5] != "issues" {
        anyhow::bail!(
            "URL must point to a GitHub issue.\n\
             Expected: https://github.com/owner/repo/issues/123\n\
             Got: {}",
            url
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

    Ok((owner, repo, number))
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
        .with_context(|| format!("Failed to fetch issue #{} from {}/{}", number, owner, repo))?;

    // Fetch comments (limited to first page)
    let comments_page = client
        .issues(owner, repo)
        .list_comments(number)
        .per_page(5)
        .send()
        .await
        .with_context(|| format!("Failed to fetch comments for issue #{}", number))?;

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
        .with_context(|| format!("Failed to post comment to issue #{}", number))?;

    let comment_url = comment.html_url.to_string();

    debug!(url = %comment_url, "Comment posted successfully");

    Ok(comment_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_issue_url() {
        let url = "https://github.com/block/goose/issues/1234";
        let (owner, repo, number) = parse_issue_url(url).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 1234);
    }

    #[test]
    fn parse_issue_url_with_fragment() {
        let url = "https://github.com/astral-sh/ruff/issues/42#issuecomment-123456";
        let (owner, repo, number) = parse_issue_url(url).unwrap();
        assert_eq!(owner, "astral-sh");
        assert_eq!(repo, "ruff");
        assert_eq!(number, 42);
    }

    #[test]
    fn parse_issue_url_with_trailing_slash() {
        let url = "https://github.com/owner/repo/issues/1/";
        let (owner, repo, number) = parse_issue_url(url).unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(number, 1);
    }

    #[test]
    fn parse_issue_url_invalid_format() {
        let url = "https://github.com/owner/repo";
        let result = parse_issue_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid GitHub issue URL"));
    }

    #[test]
    fn parse_issue_url_not_github() {
        let url = "https://gitlab.com/owner/repo/issues/1";
        let result = parse_issue_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be a GitHub issue URL"));
    }

    #[test]
    fn parse_issue_url_not_issues_path() {
        let url = "https://github.com/owner/repo/pull/1";
        let result = parse_issue_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must point to a GitHub issue"));
    }

    #[test]
    fn parse_issue_url_invalid_number() {
        let url = "https://github.com/owner/repo/issues/abc";
        let result = parse_issue_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid issue number"));
    }
}
