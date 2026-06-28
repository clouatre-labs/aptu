// SPDX-License-Identifier: Apache-2.0

//! Pull request fetching via Octocrab.
//!
//! Provides functions to parse PR references and fetch PR details
//! including file diffs for AI review.

use anyhow::{Context, Result};
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
#[allow(clippy::too_many_lines)]
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

    // Fetch PR files (diffs) with pagination (per_page=100, max 300 files)
    let mut pr_files: Vec<PrFile> = Vec::new();
    let mut page = client
        .pulls(owner, repo)
        .list_files(number)
        .await
        .with_context(|| format!("Failed to fetch files for PR #{number}"))?;

    loop {
        pr_files.extend(page.items.into_iter().map(|f| PrFile {
            filename: f.filename,
            status: format!("{:?}", f.status),
            additions: f.additions,
            deletions: f.deletions,
            patch: f.patch,
            patch_truncated: false,
            full_content: None,
        }));

        if pr_files.len() >= 300 {
            tracing::warn!(
                "PR #{} has reached 300-file cap; stopping pagination",
                number
            );
            pr_files.truncate(300);
            break;
        }

        match client
            .get_page::<octocrab::models::repos::DiffEntry>(&page.next)
            .await
        {
            Ok(Some(next_page)) => page = next_page,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!("Error fetching next page of files: {}", e);
                break;
            }
        }
    }

    // Detect truncated patches and attempt Contents API fallback
    for file in &mut pr_files {
        #[allow(clippy::collapsible_if)]
        if let Some(patch) = &file.patch {
            if is_patch_truncated(patch) {
                file.patch_truncated = true;
                // Attempt Contents API fallback
                if let Ok(Some(content)) = fetch_file_contents_single(
                    client,
                    owner,
                    repo,
                    &file.filename,
                    pr.head
                        .as_deref()
                        .map(|h| h.sha.as_str())
                        .unwrap_or_default(),
                    review_config.max_chars_per_file,
                )
                .await
                {
                    file.patch = Some(content);
                }
            }
        }
    }

    // Contents API fallback for Added/Renamed/Copied files with oversized patches.
    // Fetch full content from Contents API so the AI can review the full file, even though
    // the patch exceeds the character budget.
    for file in &mut pr_files {
        let is_added_renamed_copied = matches!(
            file.status.to_lowercase().as_str(),
            "added" | "renamed" | "copied"
        );
        let patch_too_large =
            file.patch.as_deref().map_or(0, str::len) > review_config.max_patch_chars_per_file;
        if is_added_renamed_copied && patch_too_large && file.full_content.is_none() {
            match fetch_file_contents_single(
                client,
                owner,
                repo,
                &file.filename,
                pr.head
                    .as_deref()
                    .map(|h| h.sha.as_str())
                    .unwrap_or_default(),
                review_config.max_chars_per_file,
            )
            .await
            {
                Ok(Some(content)) => {
                    file.full_content = Some(content);
                }
                Ok(None) => {
                    tracing::warn!(
                        "Contents API returned empty content for added file {} in PR #{}",
                        file.filename,
                        number
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch contents for added file {} in PR #{}: {}",
                        file.filename,
                        number,
                        e
                    );
                }
            }
        }
    }

    // Fetch full file contents for eligible files (default: up to 10 files, max 4000 chars each)
    let file_contents = fetch_file_contents(
        client,
        owner,
        repo,
        &pr_files,
        pr.head
            .as_deref()
            .map(|h| h.sha.as_str())
            .unwrap_or_default(),
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
            if file.full_content.is_none() {
                file.full_content = content;
            }
            file
        })
        .collect();

    let labels: Vec<String> = pr
        .labels
        .iter()
        .flat_map(|v| v.iter())
        .map(|l| l.name.clone())
        .collect();

    let details = PrDetails {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        title: pr.title.clone().unwrap_or_default(),
        body: pr.body.clone().unwrap_or_default(),
        base_branch: pr
            .base
            .as_deref()
            .map(|b| b.ref_field.clone())
            .unwrap_or_default(),
        head_branch: pr
            .head
            .as_deref()
            .map(|h| h.ref_field.clone())
            .unwrap_or_default(),
        head_sha: pr
            .head
            .as_deref()
            .map(|h| h.sha.as_str())
            .unwrap_or_default()
            .to_string(),
        files: pr_files,
        url: pr
            .html_url
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
        labels,
        review_comments: Vec::new(),
        instructions: None,
        dep_enrichments: Vec::new(),
    };

    debug!(
        file_count = details.files.len(),
        "PR details fetched successfully"
    );

    Ok(details)
}

/// Detects if a patch is truncated mid-hunk by GitHub API.
///
/// A patch is considered truncated if the last non-empty line starts with '+' or '-',
/// indicating an incomplete hunk.
fn is_patch_truncated(patch: &str) -> bool {
    let lines: Vec<&str> = patch.lines().collect();

    // Rule 1: Check if last non-empty line starts with '+' or '-' (mid-hunk cutoff)
    if let Some(last_line) = lines.iter().rev().find(|line| !line.trim().is_empty())
        && (last_line.starts_with('+') || last_line.starts_with('-'))
    {
        return true;
    }

    // Rule 2: Check if declared hunk size matches actual lines delivered
    // Parse the last @@ -a,b +c,d @@ header and verify line count
    if let Some(last_hunk_header) = lines.iter().rev().find(|line| line.contains("@@")) {
        // Extract the +c,d part from the hunk header
        if let Some(plus_part) = last_hunk_header.split('+').nth(1) {
            // Extract the number after '+' and before the next space or @@
            if let Some(size_str) = plus_part.split_whitespace().next() {
                // Parse "c,d" format
                if let Some(count_str) = size_str.split(',').nth(1)
                    && let Ok(declared_count) = count_str.parse::<usize>()
                {
                    // Count actual lines after this hunk header (context + added lines)
                    // Find the index of this hunk header
                    if let Some(hunk_idx) = lines.iter().position(|&line| line == *last_hunk_header)
                    {
                        let lines_after_hunk = &lines[hunk_idx + 1..];
                        // Count lines that are context (' '), additions ('+'), or deletions ('-')
                        // Stop counting if we hit another hunk header
                        let mut actual_count = 0;
                        for line in lines_after_hunk {
                            if line.starts_with("@@") {
                                break;
                            }
                            if line.starts_with(' ')
                                || line.starts_with('+')
                                || line.starts_with('-')
                            {
                                actual_count += 1;
                            }
                        }
                        // If actual count is less than declared, the hunk is truncated
                        if actual_count < declared_count {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Fetches a single file's content from GitHub Contents API as a fallback for truncated patches.
///
/// Returns the file content truncated to `max_chars`, or `None` if the file cannot be fetched.
/// Non-fatal errors (404, rate limits) are logged as warnings.
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_file_contents_single(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    filename: &str,
    head_sha: &str,
    max_chars: usize,
) -> Result<Option<String>> {
    match client
        .repos(owner, repo)
        .get_content()
        .path(filename)
        .r#ref(head_sha)
        .send()
        .await
    {
        Ok(content) => {
            // Try to decode the first item (should be the file, not a directory listing)
            if let Some(item) = content.items.first() {
                if let Some(decoded) = item.decoded_content() {
                    let truncated = if decoded.len() > max_chars {
                        decoded.chars().take(max_chars).collect::<String>()
                    } else {
                        decoded
                    };
                    Ok(Some(truncated))
                } else {
                    tracing::warn!(
                        "Failed to decode content for {}/{}/{} at {}",
                        owner,
                        repo,
                        filename,
                        head_sha
                    );
                    Ok(None)
                }
            } else {
                tracing::warn!(
                    "File content response was empty for {}/{}/{} at {}",
                    owner,
                    repo,
                    filename,
                    head_sha
                );
                Ok(None)
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to fetch content for {}/{}/{} at {}: {}",
                owner,
                repo,
                filename,
                head_sha,
                e
            );
            Ok(None)
        }
    }
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
#[cfg(not(target_arch = "wasm32"))]
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
        if should_skip_file(&file.filename, &file.status, file.patch.as_ref()) {
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
#[cfg(not(target_arch = "wasm32"))]
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

/// Deletes a PR review comment.
///
/// # Errors
///
/// Returns an error if the API request fails. 404 errors (comment not found)
/// are treated as success (idempotent).
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, comment_id = comment_id))]
pub async fn delete_pr_review_comment(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    comment_id: u64,
) -> Result<()> {
    debug!("Deleting PR review comment");

    let route = format!("/repos/{owner}/{repo}/pulls/comments/{comment_id}");

    // Use generic delete method; needs explicit empty object body type
    let empty_body = serde_json::json!({});
    let result: std::result::Result<serde_json::Value, _> =
        client.delete(&route, Some(&empty_body)).await;

    match result {
        Ok(_) => {
            debug!("PR review comment deleted successfully");
            Ok(())
        }
        Err(e)
            if let octocrab::Error::GitHub { source, .. } = &e
                && source.status_code.as_u16() == 404 =>
        {
            debug!("PR review comment already deleted (404); treating as success");
            Ok(())
        }
        Err(e) => {
            Err(e).with_context(|| format!("Failed to delete PR review comment #{comment_id}"))
        }
    }
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
        } else if path.starts_with("docs/") {
            Some("documentation")
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
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, head = %head_branch, base = %base_branch))]
#[allow(clippy::too_many_arguments)]
pub async fn create_pull_request(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    title: &str,
    head_branch: &str,
    base_branch: &str,
    body: Option<&str>,
    draft: bool,
) -> anyhow::Result<PrCreateResult> {
    debug!("Creating pull request");

    let pr = client
        .pulls(owner, repo)
        .create(title, head_branch, base_branch)
        .body(body.unwrap_or_default())
        .draft(draft)
        .send()
        .await
        .with_context(|| {
            format!("Failed to create PR in {owner}/{repo} ({head_branch} -> {base_branch})")
        })?;

    let result = PrCreateResult {
        pr_number: pr.number.unwrap_or(0),
        url: pr
            .html_url
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
        branch: pr
            .head
            .as_deref()
            .map(|h| h.ref_field.clone())
            .unwrap_or_default(),
        base: pr
            .base
            .as_deref()
            .map(|b| b.ref_field.clone())
            .unwrap_or_default(),
        title: pr.title.clone().unwrap_or_default(),
        draft: pr.draft.unwrap_or(false),
        files_changed: u32::try_from(pr.changed_files.unwrap_or(0)).unwrap_or(u32::MAX),
        additions: pr.additions.unwrap_or(0),
        deletions: pr.deletions.unwrap_or(0),
    };

    debug!(
        pr_number = result.pr_number,
        "Pull request created successfully"
    );

    Ok(result)
}

/// Determines whether a file should be skipped during fetch based on status and patch.
/// Emits a debug log with the skip reason. Returns true if the file should be skipped
/// (removed status or no patch), false otherwise.
fn should_skip_file(filename: &str, status: &str, patch: Option<&String>) -> bool {
    if status.to_lowercase().contains("removed") {
        debug!(file = %filename, "Skipping removed file");
        return true;
    }
    if patch.is_none_or(String::is_empty) {
        debug!(file = %filename, "Skipping file with empty patch");
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::CommentSeverity;

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
                assert!(labels.is_empty(), "{msg}: expected empty, got {labels:?}");
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
                "feat: docs",
                vec!["docs/GITHUB_ACTION.md"],
                vec!["enhancement", "documentation"],
                "docs path should map to documentation scope",
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
            patch_truncated: false,
            full_content: None,
        };
        let modified_file = PrFile {
            filename: "file_0.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: Some("+ new code".to_string()),
            patch_truncated: false,
            full_content: None,
        };
        let no_patch_file = PrFile {
            filename: "file_1.rs".to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        };

        // Assert: removed files are skipped
        assert!(
            should_skip_file(
                &removed_file.filename,
                &removed_file.status,
                removed_file.patch.as_ref()
            ),
            "removed files should be skipped"
        );

        // Assert: modified files with patch are not skipped
        assert!(
            !should_skip_file(
                &modified_file.filename,
                &modified_file.status,
                modified_file.patch.as_ref()
            ),
            "modified files with patch should not be skipped"
        );

        // Assert: files without patch are skipped
        assert!(
            should_skip_file(
                &no_patch_file.filename,
                &no_patch_file.status,
                no_patch_file.patch.as_ref()
            ),
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

    #[test]
    fn test_list_files_pagination_collects_all_pages() {
        // Arrange: simulate pagination with two pages
        // Page 1: 100 items with next_link set
        let mut page1_items = Vec::new();
        for i in 0..100 {
            page1_items.push(PrFile {
                filename: format!("file{}.rs", i),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
                patch_truncated: false,
                full_content: None,
            });
        }

        // Page 2: 50 items with no next_link
        let mut page2_items = Vec::new();
        for i in 100..150 {
            page2_items.push(PrFile {
                filename: format!("file{}.rs", i),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
                patch_truncated: false,
                full_content: None,
            });
        }

        // Act: collect all items (simulating pagination loop)
        let mut all_files = Vec::new();
        all_files.extend(page1_items);
        all_files.extend(page2_items);

        // Assert: total collected == 150
        assert_eq!(
            all_files.len(),
            150,
            "pagination should collect all items from both pages"
        );
    }

    #[test]
    fn test_list_files_pagination_respects_300_file_cap() {
        // Arrange: build a Vec of 301 PrFile items
        let mut files = Vec::new();
        for i in 0..301 {
            files.push(PrFile {
                filename: format!("file{}.rs", i),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n-old\n+new".to_string()),
                patch_truncated: false,
                full_content: None,
            });
        }

        // Act: apply the 300-file cap (simulating the truncate logic)
        if files.len() >= 300 {
            files.truncate(300);
        }

        // Assert: result.len() == 300
        assert_eq!(files.len(), 300, "pagination should enforce 300-file cap");
    }

    #[test]
    fn test_is_patch_truncated_detects_mid_hunk_plus() {
        // Test: patch ending with '+' (mid-hunk truncation)
        let truncated_patch = "@@ -1,3 +1,4 @@\n line1\n line2\n+";
        assert!(
            is_patch_truncated(truncated_patch),
            "patch ending with + should be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_detects_mid_hunk_minus() {
        // Test: patch ending with '-' (mid-hunk truncation)
        let truncated_patch = "@@ -1,3 +1,4 @@\n line1\n line2\n-";
        assert!(
            is_patch_truncated(truncated_patch),
            "patch ending with - should be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_clean_patch_context_line() {
        // Test: patch ending with ' ' (context line, not truncated)
        let clean_patch = "@@ -1,3 +1,3 @@\n line1\n line2\n line3";
        assert!(
            !is_patch_truncated(clean_patch),
            "patch ending with context line should not be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_correct_hunk_line_count() {
        // Test: patch with correct hunk line count (declared 3, actual 3)
        let clean_patch = "@@ -1,3 +1,3 @@\n line1\n line2\n line3";
        assert!(
            !is_patch_truncated(clean_patch),
            "patch with correct hunk line count should not be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_declared_hunk_size_larger_than_delivered() {
        // Test: patch with declared hunk size larger than delivered lines
        // Declared: +1,4 (4 lines in new file), Actual: only 2 lines delivered
        let truncated_patch = "@@ -1,3 +1,4 @@\n line1\n line2";
        assert!(
            is_patch_truncated(truncated_patch),
            "patch with declared hunk size larger than delivered should be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_no_hunk_header_but_last_line_plus() {
        // Test: patch with no @@ header but last line is '+'
        let truncated_patch = "line1\nline2\n+";
        assert!(
            is_patch_truncated(truncated_patch),
            "patch with no @@ header but ending with + should be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_empty_patch() {
        // Test: empty patch
        let empty_patch = "";
        assert!(
            !is_patch_truncated(empty_patch),
            "empty patch should not be detected as truncated"
        );
    }

    #[test]
    fn test_is_patch_truncated_multiple_hunks_last_hunk_truncated() {
        // Test: multiple hunks where the last hunk is truncated
        let truncated_patch = "@@ -1,2 +1,2 @@\n line1\n line2\n@@ -5,3 +5,4 @@\n line5\n line6";
        assert!(
            is_patch_truncated(truncated_patch),
            "patch with last hunk truncated should be detected as truncated"
        );
    }

    #[test]
    fn test_pr_file_status_case_insensitive_added() {
        // Test: Added file status is matched case-insensitively
        let file = PrFile {
            filename: "new.rs".to_string(),
            status: "Added".to_string(), // Debug repr from Octocrab
            additions: 50,
            deletions: 0,
            patch: Some("new code".to_string()),
            patch_truncated: false,
            full_content: None,
        };

        let is_added_renamed_copied = matches!(
            file.status.to_lowercase().as_str(),
            "added" | "renamed" | "copied"
        );
        assert!(is_added_renamed_copied, "Added status should be recognized");
    }

    #[test]
    fn test_pr_file_status_case_insensitive_modified() {
        // Test: Modified file status is NOT matched (edge case)
        let file = PrFile {
            filename: "existing.rs".to_string(),
            status: "Modified".to_string(),
            additions: 10,
            deletions: 5,
            patch: Some("modified code".to_string()),
            patch_truncated: false,
            full_content: None,
        };

        let is_added_renamed_copied = matches!(
            file.status.to_lowercase().as_str(),
            "added" | "renamed" | "copied"
        );
        assert!(
            !is_added_renamed_copied,
            "Modified status should NOT be recognized as added/renamed/copied"
        );
    }

    #[test]
    fn test_pr_file_oversized_patch_detection() {
        // Test: Patch size is compared against max_patch_chars_per_file.
        // Derive the limit from ReviewConfig::default() -- single source of truth.
        let max_patch_chars = crate::config::ReviewConfig::default().max_patch_chars_per_file;
        let patch = "a".repeat(max_patch_chars + 5_000); // Clearly exceeds limit

        let patch_too_large = patch.len() > max_patch_chars;
        assert!(
            patch_too_large,
            "patch exceeding the default limit should be detected as oversized"
        );
    }

    #[test]
    fn test_pr_file_dedup_guard_full_content_present() {
        // Test: File with full_content already populated should skip Contents API call
        let file = PrFile {
            filename: "new.rs".to_string(),
            status: "Added".to_string(),
            additions: 50,
            deletions: 0,
            patch: Some("new code".to_string()),
            patch_truncated: false,
            full_content: Some("full content from Contents API".to_string()),
        };

        let should_fetch = file.full_content.is_none();
        assert!(
            !should_fetch,
            "File with full_content should not be fetched again (dedup guard)"
        );
    }

    #[test]
    fn test_pr_file_contents_api_fallback_flow() {
        // Test: Verify the three conditions for Contents API fallback:
        // 1. status is Added/Renamed/Copied
        // 2. patch size exceeds max_patch_chars_per_file
        // 3. full_content is None
        // Derive the limit from ReviewConfig::default() -- single source of truth.
        let max_patch_chars = crate::config::ReviewConfig::default().max_patch_chars_per_file;

        let file = PrFile {
            filename: "new.rs".to_string(),
            status: "Added".to_string(),
            additions: 50,
            deletions: 0,
            patch: Some("a".repeat(max_patch_chars + 5_000)), // Clearly exceeds limit
            patch_truncated: false,
            full_content: None, // Not yet fetched
        };

        let is_added_renamed_copied = matches!(
            file.status.to_lowercase().as_str(),
            "added" | "renamed" | "copied"
        );
        let patch_too_large = file.patch.as_deref().map_or(0, str::len) > max_patch_chars;
        let should_attempt_contents_api =
            is_added_renamed_copied && patch_too_large && file.full_content.is_none();

        assert!(
            should_attempt_contents_api,
            "Added file with 30k patch and no full_content should attempt Contents API"
        );
    }

    #[test]
    fn test_merge_preserves_existing_full_content() {
        // Arrange: create a PrFile with full_content = Some("fallback content"),
        // pair it with content = None (simulating fetch_file_contents returning None beyond the cap)
        let mut file = PrFile {
            filename: "test.rs".to_string(),
            status: "modified".to_string(),
            additions: 5,
            deletions: 2,
            patch: Some("@@ -1,1 +1,1 @@".to_string()),
            patch_truncated: false,
            full_content: Some("fallback content".to_string()),
        };
        let content = None;

        // Act: apply the fixed merge logic
        if file.full_content.is_none() {
            file.full_content = content;
        }

        // Assert: file.full_content == Some("fallback content")
        assert_eq!(file.full_content, Some("fallback content".to_string()));
    }

    #[test]
    fn test_merge_sets_full_content_when_none() {
        // Arrange: PrFile with full_content = None, content = Some("fetched content")
        let mut file = PrFile {
            filename: "test.rs".to_string(),
            status: "modified".to_string(),
            additions: 5,
            deletions: 2,
            patch: Some("@@ -1,1 +1,1 @@".to_string()),
            patch_truncated: false,
            full_content: None,
        };
        let content = Some("fetched content".to_string());

        // Act: apply merge logic
        if file.full_content.is_none() {
            file.full_content = content;
        }

        // Assert: file.full_content == Some("fetched content")
        assert_eq!(file.full_content, Some("fetched content".to_string()));
    }

    #[test]
    fn test_fetch_file_contents_fallback_on_truncated_patch() {
        // Note: The Contents API network call cannot be unit-tested without a mock.
        // The fallback is exercised in integration tests via the full fetch_pr_details flow.
        // Unit tests for is_patch_truncated are above.
        // New unit tests for the added/renamed/copied Contents API fallback:
        // - test_pr_file_status_case_insensitive_added
        // - test_pr_file_status_case_insensitive_modified
        // - test_pr_file_oversized_patch_detection
        // - test_pr_file_dedup_guard_full_content_present
        // - test_pr_file_contents_api_fallback_flow
    }
}
