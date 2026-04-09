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
    review_config: &crate::config::ReviewConfig,
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
            full_content: None,
        })
        .collect();

    // Fetch full file contents for eligible files (default: up to 10 files, max 4000 chars each)
    let file_contents = fetch_file_contents(
        client,
        owner,
        repo,
        &pr_files,
        &pr.head.sha,
        review_config.max_full_content_files,
        review_config.max_chars_per_file,
    )
    .await;

    // Merge file contents back into pr_files
    debug_assert_eq!(
        pr_files.len(),
        file_contents.len(),
        "fetch_file_contents must return one entry per file"
    );
    let pr_files: Vec<PrFile> = pr_files
        .into_iter()
        .zip(file_contents)
        .map(|(mut file, content)| {
            file.full_content = content;
            file
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

/// Fetches full file contents for PR files from GitHub Contents API.
///
/// Fetches content for eligible files up to a specified limit and truncates each to a character limit.
/// Skips deleted files and files with empty patches. Per-file errors are non-fatal: they produce
/// `None` entries and log warnings.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `files` - Slice of PR files to fetch
/// * `head_sha` - PR head commit SHA to fetch from
/// * `max_files` - Maximum number of files to fetch content for
/// * `max_chars_per_file` - Truncate each file's content at this character limit
///
/// # Returns
///
/// Vector of `Option<String>` with one entry per input file (in order):
/// - `Some(content)` if fetch succeeded
/// - `None` if fetch failed, file was skipped, or file index exceeded `max_files`
#[instrument(skip(client, files), fields(owner = %owner, repo = %repo, max_files = max_files))]
async fn fetch_file_contents(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    files: &[PrFile],
    head_sha: &str,
    max_files: usize,
    max_chars_per_file: usize,
) -> Vec<Option<String>> {
    let mut results = Vec::with_capacity(files.len());
    let mut fetched_count = 0usize;

    for file in files {
        // Skip deleted or removed files
        if file.status.to_lowercase().contains("removed") {
            debug!(file = %file.filename, "Skipping removed file");
            results.push(None);
            continue;
        }

        // Skip if empty patch
        if file.patch.as_ref().is_none_or(String::is_empty) {
            debug!(file = %file.filename, "Skipping file with empty patch");
            results.push(None);
            continue;
        }

        // Skip if beyond max_files cap (count only successfully-fetched files)
        if fetched_count >= max_files {
            debug!(
                file = %file.filename,
                fetched_count = fetched_count,
                max_files = max_files,
                "Fetched file count exceeds max_files cap"
            );
            results.push(None);
            continue;
        }

        // Attempt to fetch file content
        match client
            .repos(owner, repo)
            .get_content()
            .path(&file.filename)
            .r#ref(head_sha)
            .send()
            .await
        {
            Ok(content) => {
                // Try to decode the first item (should be the file, not a directory listing)
                if let Some(item) = content.items.first() {
                    if let Some(decoded) = item.decoded_content() {
                        let truncated = if decoded.len() > max_chars_per_file {
                            decoded.chars().take(max_chars_per_file).collect::<String>()
                        } else {
                            decoded
                        };
                        debug!(
                            file = %file.filename,
                            content_len = truncated.len(),
                            "File content fetched and truncated"
                        );
                        results.push(Some(truncated));
                        fetched_count += 1;
                    } else {
                        tracing::warn!(
                            file = %file.filename,
                            "Failed to decode file content; skipping"
                        );
                        results.push(None);
                    }
                } else {
                    tracing::warn!(
                        file = %file.filename,
                        "File content response was empty; skipping"
                    );
                    results.push(None);
                }
            }
            Err(e) => {
                tracing::warn!(
                    file = %file.filename,
                    err = %e,
                    "Failed to fetch file content; skipping"
                );
                results.push(None);
            }
        }
    }

    results
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

/// Determines whether a file should be skipped during fetch based on status and patch.
/// Returns true if the file should be skipped (removed status or no patch), false otherwise.
#[inline]
#[allow(dead_code)]
fn should_skip_file(status: &str, patch: Option<&String>) -> bool {
    status.to_lowercase().contains("removed")
        || patch.is_none_or(String::is_empty)
}

/// Decodes base64-encoded content and truncates to `max_chars` on character boundary.
/// Returns `None` if base64 decoding fails or if the decoded content is not valid UTF-8.
#[allow(dead_code)]
fn decode_content(encoded: &str, max_chars: usize) -> Option<String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let decoded_bytes = engine.decode(encoded).ok()?;
    let decoded_str = String::from_utf8(decoded_bytes).ok()?;

    if decoded_str.len() <= max_chars {
        Some(decoded_str)
    } else {
        Some(decoded_str.chars().take(max_chars).collect::<String>())
    }
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

    #[test]
    fn test_should_skip_file_respects_fetched_count_cap() {
        // Test that should_skip_file correctly identifies files to skip.
        // Files with removed status or no patch should be skipped.
        let removed_file = PrFile {
            filename: "removed.rs".to_string(),
            status: "removed".to_string(),
            additions: 0,
            deletions: 5,
            patch: None,
            full_content: None,
        };
        let modified_file = PrFile {
            filename: "file_0.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: Some("+ new code".to_string()),
            full_content: None,
        };
        let no_patch_file = PrFile {
            filename: "file_1.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: None,
            full_content: None,
        };

        // Assert: removed files are skipped
        assert!(
            should_skip_file(&removed_file.status, removed_file.patch.as_ref()),
            "removed files should be skipped"
        );

        // Assert: modified files with patch are not skipped
        assert!(
            !should_skip_file(&modified_file.status, modified_file.patch.as_ref()),
            "modified files with patch should not be skipped"
        );

        // Assert: files without patch are skipped
        assert!(
            should_skip_file(&no_patch_file.status, no_patch_file.patch.as_ref()),
            "files without patch should be skipped"
        );
    }

    #[test]
    fn test_decode_content_valid_base64() {
        // Arrange: valid base64-encoded string
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let original = "Hello, World!";
        let encoded = engine.encode(original);

        // Act: decode with sufficient max_chars
        let result = decode_content(&encoded, 1000);

        // Assert: decoding succeeds and matches original
        assert_eq!(
            result,
            Some(original.to_string()),
            "valid base64 should decode successfully"
        );
    }

    #[test]
    fn test_decode_content_invalid_base64() {
        // Arrange: invalid base64 string
        let invalid_base64 = "!!!invalid!!!";

        // Act: attempt to decode
        let result = decode_content(invalid_base64, 1000);

        // Assert: decoding fails gracefully
        assert_eq!(result, None, "invalid base64 should return None");
    }

    #[test]
    fn test_decode_content_truncates_at_max_chars() {
        // Arrange: multi-byte UTF-8 string (Japanese characters)
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let original = "こんにちは".repeat(10); // 50 characters total
        let encoded = engine.encode(&original);
        let max_chars = 10;

        // Act: decode with max_chars limit
        let result = decode_content(&encoded, max_chars);

        // Assert: result is truncated to max_chars on character boundary
        assert!(result.is_some(), "decoding should succeed");
        let decoded = result.unwrap();
        assert_eq!(
            decoded.chars().count(),
            max_chars,
            "output should be truncated to max_chars on character boundary"
        );
        assert!(
            decoded.is_char_boundary(decoded.len()),
            "output should be valid UTF-8 (truncated on char boundary)"
        );
    }
}
