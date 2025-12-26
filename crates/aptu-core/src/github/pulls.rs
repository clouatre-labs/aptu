// SPDX-License-Identifier: Apache-2.0

//! Pull request fetching via Octocrab.
//!
//! Provides functions to parse PR references and fetch PR details
//! including file diffs for AI review.

use anyhow::{Context, Result, bail};
use octocrab::Octocrab;
use tracing::{debug, instrument};

use crate::ai::types::{PrDetails, PrFile, ReviewEvent};

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

/// Posts a PR review to GitHub.
///
/// Uses Octocrab's custom HTTP POST to create a review with the specified event type.
/// Requires write access to the repository.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `number` - PR number
/// * `body` - Review comment text
/// * `event` - Review event type (Comment, Approve, or `RequestChanges`)
///
/// # Returns
///
/// Review ID on success.
///
/// # Errors
///
/// Returns an error if the API call fails, user lacks write access, or PR is not found.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number, event = %event))]
pub async fn post_pr_review(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    body: &str,
    event: ReviewEvent,
) -> Result<u64> {
    debug!("Posting PR review");

    let route = format!("repos/{owner}/{repo}/pulls/{number}/reviews");

    let payload = serde_json::json!({
        "body": body,
        "event": event.to_string(),
    });

    #[derive(serde::Deserialize)]
    struct ReviewResponse {
        id: u64,
    }

    let response: ReviewResponse = client.post(route, Some(&payload)).await.with_context(|| {
        format!(
            "Failed to post review to PR #{number} in {owner}/{repo}. \
                 Check that you have write access to the repository."
        )
    })?;

    debug!(review_id = response.id, "PR review posted successfully");

    Ok(response.id)
}

/// Extract labels from PR metadata (title and file paths).
///
/// Parses conventional commit prefix from PR title and maps file paths to scope labels.
/// Returns a vector of label names to apply to the PR.
///
/// # Arguments
/// * `title` - PR title (may contain conventional commit prefix)
/// * `file_paths` - List of file paths changed in the PR
///
/// # Returns
/// Vector of label names to apply
#[must_use]
pub fn labels_from_pr_metadata(title: &str, file_paths: &[String]) -> Vec<String> {
    let mut labels = Vec::new();

    // Extract conventional commit prefix from title
    let prefix = title.split(':').next().unwrap_or("").trim();

    // Map conventional commit type to label
    let type_label = match prefix {
        "feat" | "perf" => Some("enhancement"),
        "fix" => Some("bug"),
        "docs" => Some("documentation"),
        "refactor" => Some("refactor"),
        _ => None,
    };

    if let Some(label) = type_label {
        labels.push(label.to_string());
    }

    // Map file paths to scope labels
    let mut scope_labels = std::collections::HashSet::new();

    for path in file_paths {
        let scope = if path.starts_with("crates/aptu-cli/") {
            Some("cli")
        } else if path.starts_with("crates/aptu-ffi/") || path.starts_with("AptuApp/") {
            Some("ios")
        } else if path.starts_with("docs/") {
            Some("documentation")
        } else if path.starts_with("snap/") {
            Some("distribution")
        } else {
            None
        };

        if let Some(label) = scope {
            scope_labels.insert(label.to_string());
        }
    }

    labels.extend(scope_labels);
    labels
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

    #[test]
    fn test_labels_from_pr_metadata_feat_prefix() {
        let labels = labels_from_pr_metadata("feat: add new feature", &[]);
        assert!(labels.contains(&"enhancement".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_fix_prefix() {
        let labels = labels_from_pr_metadata("fix: resolve bug", &[]);
        assert!(labels.contains(&"bug".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_docs_prefix() {
        let labels = labels_from_pr_metadata("docs: update readme", &[]);
        assert!(labels.contains(&"documentation".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_refactor_prefix() {
        let labels = labels_from_pr_metadata("refactor: improve code", &[]);
        assert!(labels.contains(&"refactor".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_perf_prefix() {
        let labels = labels_from_pr_metadata("perf: optimize", &[]);
        assert!(labels.contains(&"enhancement".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_ignored_prefix() {
        let labels = labels_from_pr_metadata("chore: update deps", &[]);
        assert!(labels.is_empty());
    }

    #[test]
    fn test_labels_from_pr_metadata_cli_path() {
        let labels =
            labels_from_pr_metadata("feat: cli", &["crates/aptu-cli/src/main.rs".to_string()]);
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"cli".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_ios_path_ffi() {
        let labels =
            labels_from_pr_metadata("feat: ios", &["crates/aptu-ffi/src/lib.rs".to_string()]);
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"ios".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_ios_path_app() {
        let labels =
            labels_from_pr_metadata("feat: ios", &["AptuApp/ContentView.swift".to_string()]);
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"ios".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_docs_path() {
        let labels = labels_from_pr_metadata("feat: docs", &["docs/GITHUB_ACTION.md".to_string()]);
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"documentation".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_distribution_path() {
        let labels = labels_from_pr_metadata("feat: snap", &["snap/snapcraft.yaml".to_string()]);
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"distribution".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_workflow_path_ignored() {
        let labels = labels_from_pr_metadata(
            "feat: workflow",
            &[".github/workflows/test.yml".to_string()],
        );
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(!labels.contains(&"workflow".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_multiple_paths() {
        let labels = labels_from_pr_metadata(
            "feat: multi",
            &[
                "crates/aptu-cli/src/main.rs".to_string(),
                "docs/README.md".to_string(),
            ],
        );
        assert!(labels.contains(&"enhancement".to_string()));
        assert!(labels.contains(&"cli".to_string()));
        assert!(labels.contains(&"documentation".to_string()));
    }

    #[test]
    fn test_labels_from_pr_metadata_no_prefix() {
        let labels = labels_from_pr_metadata("Random title", &[]);
        assert!(labels.is_empty());
    }
}
