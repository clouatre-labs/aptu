// SPDX-License-Identifier: Apache-2.0

//! Pull request fetching via Octocrab.
//!
//! Provides functions to parse PR references and fetch PR details
//! including file diffs for AI review.

use anyhow::{Context, Result, bail};
use octocrab::Octocrab;
use tracing::{debug, instrument};

use crate::ai::types::{PrDetails, PrFile};

/// Parses a PR reference into (owner, repo, number).
///
/// Supports multiple formats:
/// - Full URL: `https://github.com/owner/repo/pull/123`
/// - Short form: `owner/repo#123`
/// - Bare number: `123` (requires `repo_context`)
///
/// # Arguments
///
/// * `reference` - PR reference string
/// * `repo_context` - Optional repository context for bare numbers (e.g., "owner/repo")
///
/// # Returns
///
/// Tuple of (owner, repo, number)
///
/// # Errors
///
/// Returns an error if the reference format is invalid or `repo_context` is missing for bare numbers.
pub fn parse_pr_reference(
    reference: &str,
    repo_context: Option<&str>,
) -> Result<(String, String, u64)> {
    let reference = reference.trim();

    // Try full GitHub URL first
    // Format: https://github.com/owner/repo/pull/123
    if reference.starts_with("https://github.com/") || reference.starts_with("http://github.com/") {
        let path = reference
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/");

        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 4 && parts[2] == "pull" {
            let owner = parts[0].to_string();
            let repo = parts[1].to_string();
            let number: u64 = parts[3]
                .parse()
                .with_context(|| format!("Invalid PR number in URL: {}", parts[3]))?;
            return Ok((owner, repo, number));
        }
        bail!("Invalid GitHub PR URL format: {reference}");
    }

    // Try short form: owner/repo#123
    if let Some((repo_part, num_part)) = reference.split_once('#') {
        if let Some((owner, repo)) = repo_part.split_once('/') {
            let number: u64 = num_part
                .parse()
                .with_context(|| format!("Invalid PR number: {num_part}"))?;
            return Ok((owner.to_string(), repo.to_string(), number));
        }
        // Just #123 with repo_context
        if let Some(ctx) = repo_context
            && let Some((owner, repo)) = ctx.split_once('/')
        {
            let number: u64 = num_part
                .parse()
                .with_context(|| format!("Invalid PR number: {num_part}"))?;
            return Ok((owner.to_string(), repo.to_string(), number));
        }
        bail!("Invalid PR reference format: {reference}");
    }

    // Try bare number with repo_context
    if let Ok(number) = reference.parse::<u64>() {
        if let Some(ctx) = repo_context {
            if let Some((owner, repo)) = ctx.split_once('/') {
                return Ok((owner.to_string(), repo.to_string(), number));
            }
            bail!("Invalid repo_context format, expected 'owner/repo': {ctx}");
        }
        bail!("Bare PR number requires --repo flag or default_repo config: {reference}");
    }

    bail!(
        "Invalid PR reference format: {reference}. Expected URL, owner/repo#number, or number with --repo"
    )
}

/// Fetches PR details including file diffs from GitHub.
///
/// Uses Octocrab to fetch PR metadata and file changes.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - PR number
///
/// # Returns
///
/// `PrDetails` struct with PR metadata and file diffs.
///
/// # Errors
///
/// Returns an error if the API call fails or PR is not found.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
pub async fn fetch_pr_details(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PrDetails> {
    debug!("Fetching PR details");

    // Fetch PR metadata
    let pr = client
        .pulls(owner, repo)
        .get(number)
        .await
        .with_context(|| format!("Failed to fetch PR #{number} from {owner}/{repo}"))?;

    // Fetch PR files (diffs)
    let files = client
        .pulls(owner, repo)
        .list_files(number)
        .await
        .with_context(|| format!("Failed to fetch files for PR #{number}"))?;

    // Convert to our types
    let pr_files: Vec<PrFile> = files
        .items
        .into_iter()
        .map(|f| PrFile {
            filename: f.filename,
            status: format!("{:?}", f.status),
            additions: f.additions,
            deletions: f.deletions,
            patch: f.patch,
        })
        .collect();

    let details = PrDetails {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        title: pr.title.unwrap_or_default(),
        body: pr.body.unwrap_or_default(),
        base_branch: pr.base.ref_field,
        head_branch: pr.head.ref_field,
        files: pr_files,
        url: pr.html_url.map_or_else(String::new, |u| u.to_string()),
    };

    debug!(
        file_count = details.files.len(),
        "PR details fetched successfully"
    );

    Ok(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pr_reference_full_url() {
        let (owner, repo, number) =
            parse_pr_reference("https://github.com/block/goose/pull/123", None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_parse_pr_reference_short_form() {
        let (owner, repo, number) = parse_pr_reference("block/goose#456", None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 456);
    }

    #[test]
    fn test_parse_pr_reference_bare_number_with_context() {
        let (owner, repo, number) = parse_pr_reference("789", Some("block/goose")).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 789);
    }

    #[test]
    fn test_parse_pr_reference_bare_number_without_context() {
        let result = parse_pr_reference("123", None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires --repo flag")
        );
    }

    #[test]
    fn test_parse_pr_reference_hash_with_context() {
        let (owner, repo, number) = parse_pr_reference("#42", Some("owner/repo")).unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(number, 42);
    }

    #[test]
    fn test_parse_pr_reference_invalid_url() {
        let result = parse_pr_reference("https://github.com/invalid", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pr_reference_invalid_number() {
        let result = parse_pr_reference("block/goose#abc", None);
        assert!(result.is_err());
    }
}
