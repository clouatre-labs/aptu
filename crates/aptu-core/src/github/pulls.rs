// SPDX-License-Identifier: Apache-2.0

//! Pull request fetching via Octocrab.
//!
//! Provides functions to parse PR references and fetch PR details
//! including file diffs for AI review.

use anyhow::{Context, Result};
use octocrab::Octocrab;
use tracing::{debug, instrument};

use super::{ReferenceKind, parse_github_reference};
use crate::ai::types::{PrDetails, PrFile, PrReviewComment, ReviewEvent};
use crate::error::{AptuError, ResourceType};
use crate::triage::render_pr_review_comment_body;

/// Result from creating a pull request.
#[derive(Debug, serde::Serialize)]
pub struct PrCreateResult {
    /// PR number.
    pub pr_number: u64,
    /// PR URL.
    pub url: String,
    /// Head branch.
    pub branch: String,
    /// Base branch.
    pub base: String,
    /// PR title.
    pub title: String,
    /// Whether the PR is a draft.
    pub draft: bool,
    /// Number of files changed.
    pub files_changed: u32,
    /// Number of additions.
    pub additions: u64,
    /// Number of deletions.
    pub deletions: u64,
}

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
    parse_github_reference(ReferenceKind::Pull, reference, repo_context)
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
    let pr = match client.pulls(owner, repo).get(number).await {
        Ok(pr) => pr,
        Err(e) => {
            // Check if this is a 404 error and if an issue exists instead
            if let octocrab::Error::GitHub { source, .. } = &e
                && source.status_code == 404
            {
                // Try to fetch as an issue to provide a better error message
                if (client.issues(owner, repo).get(number).await).is_ok() {
                    return Err(AptuError::TypeMismatch {
                        number,
                        expected: ResourceType::PullRequest,
                        actual: ResourceType::Issue,
                    }
                    .into());
                }
                // Issue check failed, fall back to original error
            }
            return Err(e)
                .with_context(|| format!("Failed to fetch PR #{number} from {owner}/{repo}"));
        }
    };

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

    let labels: Vec<String> = pr
        .labels
        .iter()
        .flat_map(|labels_vec| labels_vec.iter().map(|l| l.name.clone()))
        .collect();

    let details = PrDetails {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        title: pr.title.unwrap_or_default(),
        body: pr.body.unwrap_or_default(),
        base_branch: pr.base.ref_field,
        head_branch: pr.head.ref_field,
        head_sha: pr.head.sha,
        files: pr_files,
        url: pr.html_url.map_or_else(String::new, |u| u.to_string()),
        labels,
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
/// * `comments` - Inline review comments to attach; entries with `line = None` are silently skipped
/// * `commit_id` - Head commit SHA to associate with the review; omitted from payload if empty
///
/// # Returns
///
/// Review ID on success.
///
/// # Errors
///
/// Returns an error if the API call fails, user lacks write access, or PR is not found.
#[allow(clippy::too_many_arguments)]
#[instrument(skip(client, comments), fields(owner = %owner, repo = %repo, number = number, event = %event))]
pub async fn post_pr_review(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    body: &str,
    event: ReviewEvent,
    comments: &[PrReviewComment],
    commit_id: &str,
) -> Result<u64> {
    debug!("Posting PR review");

    let route = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");

    // Build inline comments array; skip entries without a line number.
    let inline_comments: Vec<serde_json::Value> = comments
        .iter()
        // Comments without a line number cannot be anchored to the diff; skip silently.
        .filter_map(|c| {
            c.line.map(|line| {
                serde_json::json!({
                    "path": c.file,
                    "line": line,
                    // RIGHT = new version of the file (added/changed lines).
                    // Use line (file line number) rather than the deprecated
                    // position (diff hunk offset) so no hunk parsing is needed.
                    "side": "RIGHT",
                    "body": render_pr_review_comment_body(c),
                })
            })
        })
        .collect();

    let mut payload = serde_json::json!({
        "body": body,
        "event": event.to_string(),
        "comments": inline_comments,
    });

    // commit_id is optional; include only when non-empty.
    if !commit_id.is_empty() {
        payload["commit_id"] = serde_json::Value::String(commit_id.to_string());
    }

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
    let mut labels = std::collections::HashSet::new();

    // Extract conventional commit prefix from title
    // Handle both "feat: ..." and "feat(scope): ..." formats
    let prefix = title
        .split(':')
        .next()
        .unwrap_or("")
        .split('(')
        .next()
        .unwrap_or("")
        .trim();

    // Map conventional commit type to label
    let type_label = match prefix {
        "feat" | "perf" => Some("enhancement"),
        "fix" => Some("bug"),
        "docs" => Some("documentation"),
        "refactor" => Some("refactor"),
        _ => None,
    };

    if let Some(label) = type_label {
        labels.insert(label.to_string());
    }

    // Map file paths to scope labels
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
            labels.insert(label.to_string());
        }
    }

    labels.into_iter().collect()
}

/// Creates a pull request on GitHub.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `title` - PR title
/// * `head_branch` - Head branch (the branch with changes)
/// * `base_branch` - Base branch (the branch to merge into)
/// * `body` - Optional PR body text
///
/// # Returns
///
/// `PrCreateResult` with PR metadata.
///
/// # Errors
///
/// Returns an error if the API call fails or the user lacks write access.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, head = %head_branch, base = %base_branch))]
pub async fn create_pull_request(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    title: &str,
    head_branch: &str,
    base_branch: &str,
    body: Option<&str>,
) -> anyhow::Result<PrCreateResult> {
    debug!("Creating pull request");

    let pr = client
        .pulls(owner, repo)
        .create(title, head_branch, base_branch)
        .body(body.unwrap_or_default())
        .draft(false)
        .send()
        .await
        .with_context(|| {
            format!("Failed to create PR in {owner}/{repo} ({head_branch} -> {base_branch})")
        })?;

    let result = PrCreateResult {
        pr_number: pr.number,
        url: pr.html_url.map_or_else(String::new, |u| u.to_string()),
        branch: pr.head.ref_field,
        base: pr.base.ref_field,
        title: pr.title.unwrap_or_default(),
        draft: pr.draft.unwrap_or(false),
        files_changed: u32::try_from(pr.changed_files.unwrap_or_default()).unwrap_or(u32::MAX),
        additions: pr.additions.unwrap_or_default(),
        deletions: pr.deletions.unwrap_or_default(),
    };

    debug!(
        pr_number = result.pr_number,
        "Pull request created successfully"
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::CommentSeverity;

    #[test]
    fn test_pr_create_result_fields() {
        // Arrange / Act: construct directly (no network call needed)
        let result = PrCreateResult {
            pr_number: 42,
            url: "https://github.com/owner/repo/pull/42".to_string(),
            branch: "feat/my-feature".to_string(),
            base: "main".to_string(),
            title: "feat: add feature".to_string(),
            draft: false,
            files_changed: 3,
            additions: 100,
            deletions: 10,
        };

        // Assert
        assert_eq!(result.pr_number, 42);
        assert_eq!(result.url, "https://github.com/owner/repo/pull/42");
        assert_eq!(result.branch, "feat/my-feature");
        assert_eq!(result.base, "main");
        assert_eq!(result.title, "feat: add feature");
        assert!(!result.draft);
        assert_eq!(result.files_changed, 3);
        assert_eq!(result.additions, 100);
        assert_eq!(result.deletions, 10);
    }

    // ---------------------------------------------------------------------------
    // post_pr_review payload construction
    // ---------------------------------------------------------------------------

    /// Helper: build the inline comments JSON array using the same logic as
    /// `post_pr_review`, without making a live HTTP call.
    fn build_inline_comments(comments: &[PrReviewComment]) -> Vec<serde_json::Value> {
        comments
            .iter()
            .filter_map(|c| {
                c.line.map(|line| {
                    serde_json::json!({
                        "path": c.file,
                        "line": line,
                        "side": "RIGHT",
                        "body": render_pr_review_comment_body(c),
                    })
                })
            })
            .collect()
    }

    #[test]
    fn test_post_pr_review_payload_with_comments() {
        // Arrange
        let comments = vec![PrReviewComment {
            file: "src/main.rs".to_string(),
            line: Some(42),
            comment: "Consider using a match here.".to_string(),
            severity: CommentSeverity::Suggestion,
            suggested_code: None,
        }];

        // Act
        let inline = build_inline_comments(&comments);

        // Assert
        assert_eq!(inline.len(), 1);
        assert_eq!(inline[0]["path"], "src/main.rs");
        assert_eq!(inline[0]["line"], 42);
        assert_eq!(inline[0]["side"], "RIGHT");
        assert_eq!(inline[0]["body"], "Consider using a match here.");
    }

    #[test]
    fn test_post_pr_review_skips_none_line_comments() {
        // Arrange: one comment with a line, one without.
        let comments = vec![
            PrReviewComment {
                file: "src/lib.rs".to_string(),
                line: None,
                comment: "General file comment.".to_string(),
                severity: CommentSeverity::Info,
                suggested_code: None,
            },
            PrReviewComment {
                file: "src/lib.rs".to_string(),
                line: Some(10),
                comment: "Inline comment.".to_string(),
                severity: CommentSeverity::Warning,
                suggested_code: None,
            },
        ];

        // Act
        let inline = build_inline_comments(&comments);

        // Assert: only the comment with a line is included.
        assert_eq!(inline.len(), 1);
        assert_eq!(inline[0]["line"], 10);
    }

    #[test]
    fn test_post_pr_review_empty_comments() {
        // Arrange
        let comments: Vec<PrReviewComment> = vec![];

        // Act
        let inline = build_inline_comments(&comments);

        // Assert: empty slice produces empty array, which serializes as [].
        assert!(inline.is_empty());
        let serialized = serde_json::to_string(&inline).unwrap();
        assert_eq!(serialized, "[]");
    }

    // ---------------------------------------------------------------------------
    // Existing tests
    // ---------------------------------------------------------------------------

    // Smoke test to verify parse_pr_reference delegates correctly.
    // Comprehensive parsing tests are in github/mod.rs.
    #[test]
    fn test_parse_pr_reference_delegates_to_shared() {
        let (owner, repo, number) =
            parse_pr_reference("https://github.com/block/goose/pull/123", None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 123);
    }

    #[test]
    fn test_title_prefix_to_label_mapping() {
        let cases = vec![
            (
                "feat: add new feature",
                vec!["enhancement"],
                "feat should map to enhancement",
            ),
            ("fix: resolve bug", vec!["bug"], "fix should map to bug"),
            (
                "docs: update readme",
                vec!["documentation"],
                "docs should map to documentation",
            ),
            (
                "refactor: improve code",
                vec!["refactor"],
                "refactor should map to refactor",
            ),
            (
                "perf: optimize",
                vec!["enhancement"],
                "perf should map to enhancement",
            ),
            (
                "chore: update deps",
                vec![],
                "chore should produce no labels",
            ),
        ];

        for (title, expected_labels, msg) in cases {
            let labels = labels_from_pr_metadata(title, &[]);
            for expected in &expected_labels {
                assert!(
                    labels.contains(&expected.to_string()),
                    "{msg}: expected '{expected}' in {labels:?}",
                );
            }
            if expected_labels.is_empty() {
                assert!(labels.is_empty(), "{msg}: expected empty, got {labels:?}",);
            }
        }
    }

    #[test]
    fn test_file_path_to_scope_mapping() {
        let cases = vec![
            (
                "feat: cli",
                vec!["crates/aptu-cli/src/main.rs"],
                vec!["enhancement", "cli"],
                "cli path should map to cli scope",
            ),
            (
                "feat: ios",
                vec!["crates/aptu-ffi/src/lib.rs"],
                vec!["enhancement", "ios"],
                "ffi path should map to ios scope",
            ),
            (
                "feat: ios",
                vec!["AptuApp/ContentView.swift"],
                vec!["enhancement", "ios"],
                "app path should map to ios scope",
            ),
            (
                "feat: docs",
                vec!["docs/GITHUB_ACTION.md"],
                vec!["enhancement", "documentation"],
                "docs path should map to documentation scope",
            ),
            (
                "feat: snap",
                vec!["snap/snapcraft.yaml"],
                vec!["enhancement", "distribution"],
                "snap path should map to distribution scope",
            ),
            (
                "feat: workflow",
                vec![".github/workflows/test.yml"],
                vec!["enhancement"],
                "workflow path should be ignored",
            ),
        ];

        for (title, paths, expected_labels, msg) in cases {
            let labels = labels_from_pr_metadata(
                title,
                &paths
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>(),
            );
            for expected in expected_labels {
                assert!(
                    labels.contains(&expected.to_string()),
                    "{msg}: expected '{expected}' in {labels:?}",
                );
            }
        }
    }

    #[test]
    fn test_combined_title_and_paths() {
        let labels = labels_from_pr_metadata(
            "feat: multi",
            &[
                "crates/aptu-cli/src/main.rs".to_string(),
                "docs/README.md".to_string(),
            ],
        );
        assert!(
            labels.contains(&"enhancement".to_string()),
            "should include enhancement from feat prefix"
        );
        assert!(
            labels.contains(&"cli".to_string()),
            "should include cli from path"
        );
        assert!(
            labels.contains(&"documentation".to_string()),
            "should include documentation from path"
        );
    }

    #[test]
    fn test_no_match_returns_empty() {
        let cases = vec![
            (
                "Random title",
                vec![],
                "unrecognized prefix should return empty",
            ),
            (
                "chore: update",
                vec![],
                "ignored prefix should return empty",
            ),
        ];

        for (title, paths, msg) in cases {
            let labels = labels_from_pr_metadata(title, &paths);
            assert!(labels.is_empty(), "{msg}: got {labels:?}");
        }
    }

    #[test]
    fn test_scoped_prefix_extracts_type() {
        let labels = labels_from_pr_metadata("feat(cli): add new feature", &[]);
        assert!(
            labels.contains(&"enhancement".to_string()),
            "scoped prefix should extract type from feat(cli)"
        );
    }

    #[test]
    fn test_duplicate_labels_deduplicated() {
        let labels = labels_from_pr_metadata("docs: update", &["docs/README.md".to_string()]);
        assert_eq!(
            labels.len(),
            1,
            "should have exactly one label when title and path both map to documentation"
        );
        assert!(
            labels.contains(&"documentation".to_string()),
            "should contain documentation label"
        );
    }
}
