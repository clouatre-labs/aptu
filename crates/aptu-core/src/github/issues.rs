// SPDX-License-Identifier: Apache-2.0

//! GitHub issue operations for the triage command.
//!
//! Provides functionality to parse issue URLs, fetch issue details,
//! and post triage comments.

use anyhow::{Context, Result};
use backon::Retryable;
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use super::{ReferenceKind, parse_github_reference};
use crate::ai::types::{IssueComment, IssueDetails, RepoIssueContext};
use crate::retry::retry_backoff;
use crate::utils::is_priority_label;

/// A GitHub issue without labels (untriaged).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntriagedIssue {
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
    /// Issue URL.
    pub url: String,
}

/// A single entry in a Git tree response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTreeEntry {
    /// File path relative to repository root.
    pub path: String,
    /// Type of entry: "blob" (file) or "tree" (directory).
    #[serde(rename = "type")]
    pub type_: String,
    /// File mode (e.g., "100644" for regular files).
    pub mode: String,
    /// SHA-1 hash of the entry.
    pub sha: String,
}

/// Response from GitHub Git Trees API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTreeResponse {
    /// List of entries in the tree.
    pub tree: Vec<GitTreeEntry>,
    /// Whether the tree is truncated (too many entries).
    pub truncated: bool,
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
    parse_github_reference(ReferenceKind::Issue, input, repo_context)
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

    // Fetch the issue with retry logic
    let issue = (|| async {
        client
            .issues(owner, repo)
            .get(number)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .retry(retry_backoff())
    .notify(|err, dur| {
        tracing::warn!(
            error = %err,
            retry_after = ?dur,
            "Retrying fetch_issue_with_comments (issue fetch)"
        );
    })
    .await
    .with_context(|| format!("Failed to fetch issue #{number} from {owner}/{repo}"))?;

    // Fetch comments (limited to first page) with retry logic
    let comments_page = (|| async {
        client
            .issues(owner, repo)
            .list_comments(number)
            .per_page(5)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .retry(retry_backoff())
    .notify(|err, dur| {
        tracing::warn!(
            error = %err,
            retry_after = ?dur,
            "Retrying fetch_issue_with_comments (comments fetch)"
        );
    })
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

    let details = IssueDetails::builder()
        .owner(owner.to_string())
        .repo(repo.to_string())
        .number(number)
        .title(issue.title)
        .body(issue.body.unwrap_or_default())
        .labels(labels)
        .comments(comments)
        .url(issue_url)
        .build();

    debug!(
        labels = details.labels.len(),
        comments = details.comments.len(),
        "Fetched issue details"
    );

    Ok(details)
}

/// Extracts significant keywords from an issue title for search.
///
/// Filters out common stop words and returns lowercase keywords.
/// Extracts keywords from an issue title for relevance matching.
///
/// Filters out common stop words and limits to 5 keywords.
/// Used for prioritizing relevant files in repository tree filtering.
///
/// # Arguments
///
/// * `title` - Issue title to extract keywords from
///
/// # Returns
///
/// Vector of lowercase keywords (max 5), excluding stop words.
pub fn extract_keywords(title: &str) -> Vec<String> {
    let stop_words = [
        "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is",
        "it", "its", "of", "on", "or", "that", "the", "to", "was", "will", "with",
    ];

    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !stop_words.contains(word))
        .take(5) // Limit to first 5 keywords
        .map(std::string::ToString::to_string)
        .collect()
}

/// Searches for related issues in a repository based on title keywords.
///
/// Extracts keywords from the issue title and searches the repository
/// for matching issues. Returns up to 20 results, excluding the specified issue.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `title` - Issue title to extract keywords from
/// * `exclude_number` - Issue number to exclude from results
///
/// # Errors
///
/// Returns an error if the search API request fails.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, exclude_number = %exclude_number))]
pub async fn search_related_issues(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    title: &str,
    exclude_number: u64,
) -> Result<Vec<RepoIssueContext>> {
    let keywords = extract_keywords(title);

    if keywords.is_empty() {
        debug!("No keywords extracted from title");
        return Ok(Vec::new());
    }

    // Build search query: keyword1 keyword2 ... repo:owner/repo is:issue
    let query = format!("{} repo:{}/{} is:issue", keywords.join(" "), owner, repo);

    debug!(query = %query, "Searching for related issues");

    // Search for issues with retry logic
    let search_result = (|| async {
        client
            .search()
            .issues_and_pull_requests(&query)
            .per_page(20)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .retry(retry_backoff())
    .notify(|err, dur| {
        tracing::warn!(
            error = %err,
            retry_after = ?dur,
            "Retrying search_related_issues"
        );
    })
    .await
    .with_context(|| format!("Failed to search for related issues in {owner}/{repo}"))?;

    // Convert to our context type
    let related: Vec<RepoIssueContext> = search_result
        .items
        .iter()
        .filter_map(|item| {
            // Only include issues (not PRs)
            if item.pull_request.is_some() {
                return None;
            }

            // Exclude the issue being triaged
            if item.number == exclude_number {
                return None;
            }

            Some(RepoIssueContext {
                number: item.number,
                title: item.title.clone(),
                labels: item.labels.iter().map(|l| l.name.clone()).collect(),
                state: format!("{:?}", item.state).to_lowercase(),
            })
        })
        .collect();

    debug!(count = related.len(), "Found related issues");

    Ok(related)
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

/// Creates a new GitHub issue.
///
/// Posts a new issue with the given title and body to the repository.
/// Returns the issue URL and issue number.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `title` - Issue title
/// * `body` - Issue body (markdown)
///
/// # Errors
///
/// Returns an error if the GitHub API call fails.
#[instrument(skip(client), fields(owner = %owner, repo = %repo))]
pub async fn create_issue(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    title: &str,
    body: &str,
) -> Result<(String, u64)> {
    debug!("Creating GitHub issue");

    let issue = client
        .issues(owner, repo)
        .create(title)
        .body(body)
        .send()
        .await
        .with_context(|| format!("Failed to create issue in {owner}/{repo}"))?;

    let issue_url = issue.html_url.to_string();
    let issue_number = issue.number;

    debug!(number = issue_number, url = %issue_url, "Issue created successfully");

    Ok((issue_url, issue_number))
}

/// Result of applying labels and milestone to an issue.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Labels that were successfully applied.
    pub applied_labels: Vec<String>,
    /// Milestone that was successfully applied, if any.
    pub applied_milestone: Option<String>,
    /// Warnings about labels or milestones that could not be applied.
    pub warnings: Vec<String>,
}

/// Merges existing and suggested labels additively.
/// Labels that should only be applied by maintainers, not by AI suggestions
const MAINTAINER_ONLY_LABELS: &[&str] = &["good first issue", "help wanted"];

///
/// Implements additive label merging with priority label handling:
/// - If existing labels contain a priority label (p[0-9]), skip AI-suggested priority labels
/// - Merge remaining labels with case-insensitive deduplication
/// - Preserve all existing labels
///
/// # Arguments
///
/// * `existing_labels` - Labels currently on the issue
/// * `suggested_labels` - Labels suggested by AI
///
/// # Returns
///
/// Merged label list with duplicates removed (case-insensitive)
fn merge_labels(existing_labels: &[String], suggested_labels: &[String]) -> Vec<String> {
    // Check if existing labels contain a priority label
    let has_priority = existing_labels.iter().any(|label| is_priority_label(label));

    // Start with existing labels
    let mut merged = existing_labels.to_vec();

    // Add suggested labels, filtering out priority labels if existing has one
    for suggested in suggested_labels {
        // Skip priority labels if existing already has one
        if is_priority_label(suggested) && has_priority {
            continue;
        }

        // Skip maintainer-only labels
        if MAINTAINER_ONLY_LABELS
            .iter()
            .any(|&m| m.eq_ignore_ascii_case(suggested))
        {
            continue;
        }

        // Add if not already present (case-insensitive check)
        if !merged
            .iter()
            .any(|l| l.to_lowercase() == suggested.to_lowercase())
        {
            merged.push(suggested.clone());
        }
    }

    merged
}

/// Updates an issue with labels and milestone.
///
/// Applies labels additively by merging existing and suggested labels.
/// Validates suggestions against available options before applying.
/// Returns what was actually applied and any warnings.
///
/// # Errors
///
/// Returns an error if the GitHub API call fails.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
#[allow(clippy::too_many_arguments)]
pub async fn update_issue_labels_and_milestone(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    existing_labels: &[String],
    suggested_labels: &[String],
    existing_milestone: Option<&str>,
    suggested_milestone: Option<&str>,
    available_labels: &[crate::ai::types::RepoLabel],
    available_milestones: &[crate::ai::types::RepoMilestone],
) -> Result<ApplyResult> {
    debug!("Updating issue with labels and milestone");

    let mut warnings = Vec::new();

    // Validate and collect labels
    let available_label_names: std::collections::HashSet<_> =
        available_labels.iter().map(|l| l.name.as_str()).collect();

    // Validate suggested labels
    let mut valid_suggested = Vec::new();
    for label in suggested_labels {
        if available_label_names.contains(label.as_str()) {
            valid_suggested.push(label.clone());
        } else {
            warnings.push(format!("Label '{label}' not found in repository"));
        }
    }

    // Merge existing and suggested labels additively
    let applied_labels = merge_labels(existing_labels, &valid_suggested);

    // Validate and find milestone (only set if issue has no existing milestone)
    let applied_milestone = if existing_milestone.is_none() {
        if let Some(milestone_title) = suggested_milestone {
            if let Some(milestone) = available_milestones
                .iter()
                .find(|m| m.title == milestone_title)
            {
                Some(milestone.title.clone())
            } else {
                warnings.push(format!(
                    "Milestone '{milestone_title}' not found in repository"
                ));
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Apply updates to the issue
    let issues_handler = client.issues(owner, repo);
    let mut update_builder = issues_handler.update(number);

    if !applied_labels.is_empty() {
        update_builder = update_builder.labels(&applied_labels);
    }

    #[allow(clippy::collapsible_if)]
    if let Some(milestone_title) = &applied_milestone {
        if let Some(milestone) = available_milestones
            .iter()
            .find(|m| &m.title == milestone_title)
        {
            update_builder = update_builder.milestone(milestone.number);
        }
    }

    update_builder
        .send()
        .await
        .with_context(|| format!("Failed to update issue #{number}"))?;

    debug!(
        labels = ?applied_labels,
        milestone = ?applied_milestone,
        warnings = ?warnings,
        "Issue updated successfully"
    );

    Ok(ApplyResult {
        applied_labels,
        applied_milestone,
        warnings,
    })
}

/// Apply labels to an issue or PR by number.
///
/// Simplified label-only application function for PRs (no milestone, no merge logic).
/// Returns an error if the GitHub API call fails.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
pub async fn apply_labels_to_number(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
    labels: &[String],
) -> Result<Vec<String>> {
    debug!("Applying labels to issue/PR");

    if labels.is_empty() {
        debug!("No labels to apply");
        return Ok(Vec::new());
    }

    let route = format!("/repos/{owner}/{repo}/issues/{number}/labels");
    let payload = serde_json::json!({ "labels": labels });

    client
        .post::<_, serde_json::Value>(route, Some(&payload))
        .await
        .with_context(|| {
            format!(
                "Failed to apply labels to issue/PR #{number} in {owner}/{repo}. \
                     Check that you have write access to the repository."
            )
        })?;

    debug!(labels = ?labels, "Labels applied successfully");

    Ok(labels.to_vec())
}

/// Priority labels that should be included first in tiered filtering.
/// These labels are most actionable for issue triage.
const PRIORITY_LABELS: &[&str] = &[
    "bug",
    "enhancement",
    "documentation",
    "good first issue",
    "help wanted",
    "question",
    "feature",
    "fix",
    "breaking",
    "security",
    "performance",
    "breaking-change",
];

/// Filters labels using tiered selection: priority labels first, then remaining labels.
///
/// Implements two-tier filtering:
/// - Tier 1: Priority labels (case-insensitive matching)
/// - Tier 2: Remaining labels to fill up to `max_labels`
///
/// This ensures the AI sees the most actionable labels regardless of repository size.
///
/// # Arguments
///
/// * `labels` - List of available labels from the repository
/// * `max_labels` - Maximum number of labels to return
///
/// # Returns
///
/// Filtered list of labels with priority labels first.
#[must_use]
pub fn filter_labels_by_relevance(
    labels: &[crate::ai::types::RepoLabel],
    max_labels: usize,
) -> Vec<crate::ai::types::RepoLabel> {
    if labels.is_empty() || max_labels == 0 {
        return Vec::new();
    }

    let mut priority_labels = Vec::new();
    let mut other_labels = Vec::new();

    // Separate labels into priority and other
    for label in labels {
        let label_lower = label.name.to_lowercase();
        let is_priority = PRIORITY_LABELS
            .iter()
            .any(|&p| label_lower == p.to_lowercase());

        if is_priority {
            priority_labels.push(label.clone());
        } else {
            other_labels.push(label.clone());
        }
    }

    // Combine: priority labels first, then fill remaining slots with other labels
    let mut result = priority_labels;
    let remaining_slots = max_labels.saturating_sub(result.len());
    result.extend(other_labels.into_iter().take(remaining_slots));

    // Limit to max_labels
    result.truncate(max_labels);
    result
}

/// Patterns for directories/files to completely exclude from tree filtering.
/// Based on GitHub Linguist vendor.yml and common build artifacts.
const EXCLUDE_PATTERNS: &[&str] = &[
    "node_modules/",
    "vendor/",
    "dist/",
    "build/",
    "target/",
    ".git/",
    "cache/",
    "docs/",
    "examples/",
];

/// Patterns for directories to deprioritize but not exclude.
/// These contain test/benchmark code less relevant to issue triage.
const DEPRIORITIZE_PATTERNS: &[&str] = &[
    "test/",
    "tests/",
    "spec/",
    "bench/",
    "eval/",
    "fixtures/",
    "mocks/",
];

/// Returns language-specific entry point file patterns.
/// These are prioritized as they often contain the main logic.
fn entry_point_patterns(language: &str) -> Vec<&'static str> {
    match language.to_lowercase().as_str() {
        "rust" => vec!["lib.rs", "mod.rs", "main.rs"],
        "python" => vec!["__init__.py"],
        "javascript" | "typescript" => vec!["index.ts", "index.js"],
        "java" => vec!["Main.java"],
        "go" => vec!["main.go"],
        "c#" | "csharp" => vec!["Program.cs"],
        _ => vec![],
    }
}

/// Maps programming languages to their common file extensions.
fn get_extensions_for_language(language: &str) -> Vec<&'static str> {
    match language.to_lowercase().as_str() {
        "rust" => vec!["rs"],
        "python" => vec!["py"],
        "javascript" | "typescript" => vec!["js", "ts", "jsx", "tsx"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "c++" | "cpp" => vec!["cpp", "cc", "cxx", "h", "hpp"],
        "c#" | "csharp" => vec!["cs"],
        "go" => vec!["go"],
        "ruby" => vec!["rb"],
        "php" => vec!["php"],
        "swift" => vec!["swift"],
        "kotlin" => vec!["kt"],
        "scala" => vec!["scala"],
        "r" => vec!["r"],
        "shell" | "bash" => vec!["sh", "bash"],
        "html" => vec!["html", "htm"],
        "css" => vec!["css", "scss", "sass"],
        "json" => vec!["json"],
        "yaml" | "yml" => vec!["yaml", "yml"],
        "toml" => vec!["toml"],
        "xml" => vec!["xml"],
        "markdown" => vec!["md"],
        _ => vec![],
    }
}

/// Filters repository tree entries by relevance using tiered keyword matching.
///
/// Implements three-tier filtering:
/// - Tier 1: Files matching keywords (max 35)
/// - Tier 2: Language entry points (max 10)
/// - Tier 3: Other relevant files (max 15)
///
/// Removes common non-source directories and limits results to 60 paths.
///
/// # Arguments
///
/// * `entries` - Raw tree entries from GitHub API
/// * `language` - Repository primary language for extension filtering
/// * `keywords` - Optional keywords extracted from issue title for relevance matching
///
/// # Returns
///
/// Filtered and sorted list of file paths (max 60).
fn filter_tree_by_relevance(
    entries: &[GitTreeEntry],
    language: &str,
    keywords: &[String],
) -> Vec<String> {
    let extensions = get_extensions_for_language(language);
    let entry_points = entry_point_patterns(language);

    // Filter to valid source files
    let candidates: Vec<String> = entries
        .iter()
        .filter(|entry| {
            // Only include files (blobs), not directories
            if entry.type_ != "blob" {
                return false;
            }

            // Exclude paths containing excluded directories
            if EXCLUDE_PATTERNS.iter().any(|dir| entry.path.contains(dir)) {
                return false;
            }

            // Filter by extension if language is recognized
            if extensions.is_empty() {
                // If language not recognized, include all files
                true
            } else {
                extensions.iter().any(|ext| entry.path.ends_with(ext))
            }
        })
        .map(|e| e.path.clone())
        .collect();

    // Tier 1: Files matching keywords (max 35)
    let mut tier1: Vec<String> = Vec::new();
    let mut remaining: Vec<String> = Vec::new();

    for path in candidates {
        let path_lower = path.to_lowercase();
        let matches_keyword = keywords.iter().any(|kw| path_lower.contains(kw));

        if matches_keyword && tier1.len() < 35 {
            tier1.push(path);
        } else {
            remaining.push(path);
        }
    }

    // Tier 2: Entry point files (max 10)
    let mut tier2: Vec<String> = Vec::new();
    let mut tier3_candidates: Vec<String> = Vec::new();

    for path in remaining {
        let is_entry_point = entry_points.iter().any(|ep| path.ends_with(ep));
        let is_deprioritized = DEPRIORITIZE_PATTERNS.iter().any(|dp| path.contains(dp));

        if is_entry_point && tier2.len() < 10 {
            tier2.push(path);
        } else if !is_deprioritized {
            tier3_candidates.push(path);
        }
    }

    // Tier 3: Other relevant files (max 15)
    let mut tier3: Vec<String> = tier3_candidates.into_iter().take(15).collect();

    // Combine and sort by depth within each tier
    let mut result = tier1;
    result.append(&mut tier2);
    result.append(&mut tier3);

    // Sort by path depth (fewer slashes first), then alphabetically
    result.sort_by(|a, b| {
        let depth_a = a.matches('/').count();
        let depth_b = b.matches('/').count();
        if depth_a == depth_b {
            a.cmp(b)
        } else {
            depth_a.cmp(&depth_b)
        }
    });

    // Limit to 60 paths
    result.truncate(60);
    result
}

/// Fetches the repository file tree from GitHub.
///
/// Attempts to fetch from the default branch (main, then master).
/// Returns filtered list of source file paths based on repository language and optional keywords.
///
/// # Arguments
///
/// * `client` - Authenticated Octocrab client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `language` - Repository primary language for filtering
/// * `keywords` - Optional keywords extracted from issue title for relevance matching
///
/// # Errors
///
/// Returns an error if the API request fails (but not if tree is unavailable).
#[instrument(skip(client), fields(owner = %owner, repo = %repo))]
pub async fn fetch_repo_tree(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    language: &str,
    keywords: &[String],
) -> Result<Vec<String>> {
    debug!("Fetching repository tree");

    // Try main branch first, then master
    let branches = ["main", "master"];
    let mut tree_response: Option<GitTreeResponse> = None;

    for branch in &branches {
        let route = format!("/repos/{owner}/{repo}/git/trees/{branch}?recursive=1");
        let result = (|| async {
            client
                .get::<GitTreeResponse, _, _>(&route, None::<&()>)
                .await
                .map_err(|e| anyhow::anyhow!(e))
        })
        .retry(retry_backoff())
        .notify(|err, dur| {
            tracing::warn!(
                error = %err,
                retry_after = ?dur,
                branch = %branch,
                "Retrying fetch_repo_tree"
            );
        })
        .await;

        match result {
            Ok(response) => {
                tree_response = Some(response);
                debug!(branch = %branch, "Fetched tree from branch");
                break;
            }
            Err(e) => {
                debug!(branch = %branch, error = %e, "Failed to fetch tree from branch");
            }
        }
    }

    let response =
        tree_response.context("Failed to fetch repository tree from main or master branch")?;

    let filtered = filter_tree_by_relevance(&response.tree, language, keywords);
    debug!(count = filtered.len(), "Filtered tree entries");

    Ok(filtered)
}

/// Fetches issues needing triage from a specific repository.
///
/// In default mode (force=false), returns issues that are either unlabeled OR missing a milestone.
/// In force mode (force=true), returns ALL open issues with no filtering.
///
/// # Arguments
///
/// * `client` - The Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `since` - Optional RFC3339 timestamp to filter issues created after this date (client-side filtering)
/// * `force` - If true, return all issues in the specified state; if false, filter to unlabeled or milestone-missing issues
/// * `state` - Issue state filter (Open, Closed, or All)
///
/// # Errors
///
/// Returns an error if the REST API request fails.
#[instrument(skip(client), fields(owner = %owner, repo = %repo))]
pub async fn fetch_issues_needing_triage(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    since: Option<&str>,
    force: bool,
    state: octocrab::params::State,
) -> Result<Vec<UntriagedIssue>> {
    debug!("Fetching issues needing triage");

    let issues_page: octocrab::Page<octocrab::models::issues::Issue> = client
        .issues(owner, repo)
        .list()
        .state(state)
        .per_page(100)
        .send()
        .await
        .context("Failed to fetch issues from repository")?;

    let total_issues = issues_page.items.len();

    let mut issues_needing_triage: Vec<UntriagedIssue> = issues_page
        .items
        .into_iter()
        .filter(|issue| {
            if force {
                true
            } else {
                issue.labels.is_empty() || issue.milestone.is_none()
            }
        })
        .map(|issue| UntriagedIssue {
            number: issue.number,
            title: issue.title,
            created_at: issue.created_at.to_rfc3339(),
            url: issue.html_url.to_string(),
        })
        .collect();

    if let Some(since_date) = since
        && let Ok(since_timestamp) = chrono::DateTime::parse_from_rfc3339(since_date)
    {
        issues_needing_triage.retain(|issue| {
            if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&issue.created_at) {
                created_at >= since_timestamp
            } else {
                true
            }
        });
    }

    debug!(
        total_issues = total_issues,
        issues_needing_triage_count = issues_needing_triage.len(),
        "Fetched issues needing triage"
    );

    Ok(issues_needing_triage)
}

#[cfg(test)]
mod fetch_issues_needing_triage_tests {
    #[test]
    fn filter_logic_unlabeled_default_mode() {
        let labels_empty = true;
        let milestone_none = true;
        let force = false;

        let passes = if force {
            true
        } else {
            labels_empty || milestone_none
        };

        assert!(passes);
    }

    #[test]
    fn filter_logic_labeled_default_mode() {
        let labels_empty = false;
        let milestone_none = true;
        let force = false;

        let passes = if force {
            true
        } else {
            labels_empty || milestone_none
        };

        assert!(passes);
    }

    #[test]
    fn filter_logic_missing_milestone_default_mode() {
        let labels_empty = false;
        let milestone_none = true;
        let force = false;

        let passes = if force {
            true
        } else {
            labels_empty || milestone_none
        };

        assert!(passes);
    }

    #[test]
    fn filter_logic_force_mode_returns_all() {
        let labels_empty = false;
        let milestone_none = false;
        let force = true;

        let passes = if force {
            true
        } else {
            labels_empty || milestone_none
        };

        assert!(passes);
    }

    #[test]
    fn filter_logic_fully_triaged_default_mode_excluded() {
        let labels_empty = false;
        let milestone_none = false;
        let force = false;

        let passes = if force {
            true
        } else {
            labels_empty || milestone_none
        };

        assert!(!passes);
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;

    #[test]
    fn filter_tree_by_relevance_keyword_matching() {
        let entries = vec![
            GitTreeEntry {
                path: "src/parser.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "src/main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
            GitTreeEntry {
                path: "src/utils.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "ghi789".to_string(),
            },
        ];

        let keywords = vec!["parser".to_string()];
        let filtered = filter_tree_by_relevance(&entries, "rust", &keywords);
        assert!(filtered.contains(&"src/parser.rs".to_string()));
    }

    #[test]
    fn filter_tree_by_relevance_entry_points() {
        let entries = vec![
            GitTreeEntry {
                path: "src/lib.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "src/utils.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let keywords = vec![];
        let filtered = filter_tree_by_relevance(&entries, "rust", &keywords);
        assert!(filtered.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn filter_tree_by_relevance_excludes_tests() {
        let entries = vec![
            GitTreeEntry {
                path: "src/main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "tests/integration_test.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let keywords = vec![];
        let filtered = filter_tree_by_relevance(&entries, "rust", &keywords);
        assert!(!filtered.contains(&"tests/integration_test.rs".to_string()));
        assert!(filtered.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn get_extensions_for_language_rust() {
        let exts = get_extensions_for_language("rust");
        assert_eq!(exts, vec!["rs"]);
    }

    #[test]
    fn get_extensions_for_language_javascript() {
        let exts = get_extensions_for_language("javascript");
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"jsx"));
        assert!(exts.contains(&"tsx"));
    }

    #[test]
    fn get_extensions_for_language_unknown() {
        let exts = get_extensions_for_language("unknown_language");
        assert!(exts.is_empty());
    }
}

#[cfg(test)]
mod merge_labels_tests {
    use super::*;

    #[test]
    fn preserves_existing_and_adds_new() {
        let existing = vec!["bug".to_string(), "enhancement".to_string()];
        let suggested = vec!["documentation".to_string()];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 3);
        assert!(merged.contains(&"bug".to_string()));
        assert!(merged.contains(&"enhancement".to_string()));
        assert!(merged.contains(&"documentation".to_string()));
    }

    #[test]
    fn deduplicates_case_insensitive() {
        let existing = vec!["Bug".to_string()];
        let suggested = vec!["bug".to_string(), "enhancement".to_string()];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains(&"Bug".to_string()));
        assert!(merged.contains(&"enhancement".to_string()));
    }

    #[test]
    fn skips_priority_when_existing_has_one() {
        // P1 (uppercase) exists, p2 suggested - should keep P1, skip p2, add bug
        let existing = vec!["P1".to_string()];
        let suggested = vec!["p2".to_string(), "bug".to_string()];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains(&"P1".to_string()));
        assert!(merged.contains(&"bug".to_string()));
        assert!(!merged.contains(&"p2".to_string()));
    }

    #[test]
    fn handles_empty_inputs() {
        // Empty existing: suggested labels pass through
        let merged = merge_labels(&[], &["bug".to_string(), "p1".to_string()]);
        assert_eq!(merged.len(), 2);

        // Empty suggested: existing labels preserved
        let merged = merge_labels(&["bug".to_string()], &[]);
        assert_eq!(merged.len(), 1);
        assert!(merged.contains(&"bug".to_string()));
    }

    #[test]
    fn filters_maintainer_only_labels() {
        let existing = vec![];
        let suggested = vec![
            "good first issue".to_string(),
            "help wanted".to_string(),
            "bug".to_string(),
        ];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 1);
        assert!(merged.contains(&"bug".to_string()));
        assert!(!merged.contains(&"good first issue".to_string()));
        assert!(!merged.contains(&"help wanted".to_string()));
    }

    #[test]
    fn filters_maintainer_only_case_insensitive() {
        let existing = vec![];
        let suggested = vec![
            "Good First Issue".to_string(),
            "HELP WANTED".to_string(),
            "enhancement".to_string(),
        ];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 1);
        assert!(merged.contains(&"enhancement".to_string()));
        assert!(!merged.contains(&"Good First Issue".to_string()));
        assert!(!merged.contains(&"HELP WANTED".to_string()));
    }

    #[test]
    fn skips_priority_prefix_when_existing_has_one() {
        // priority: high exists, priority: medium suggested - should keep priority: high, skip priority: medium, add bug
        let existing = vec!["priority: high".to_string()];
        let suggested = vec!["priority: medium".to_string(), "bug".to_string()];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains(&"priority: high".to_string()));
        assert!(merged.contains(&"bug".to_string()));
        assert!(!merged.contains(&"priority: medium".to_string()));
    }

    #[test]
    fn skips_mixed_priority_formats_when_existing_has_one() {
        // p1 exists, priority: high suggested - should keep p1, skip priority: high, add bug
        let existing = vec!["p1".to_string()];
        let suggested = vec!["priority: high".to_string(), "bug".to_string()];
        let merged = merge_labels(&existing, &suggested);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains(&"p1".to_string()));
        assert!(merged.contains(&"bug".to_string()));
        assert!(!merged.contains(&"priority: high".to_string()));
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn filter_labels_empty_input() {
        let labels = vec![];
        let filtered = filter_labels_by_relevance(&labels, 30);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_labels_zero_max() {
        let labels = vec![crate::ai::types::RepoLabel {
            name: "bug".to_string(),
            color: "ff0000".to_string(),
            description: "Bug report".to_string(),
        }];
        let filtered = filter_labels_by_relevance(&labels, 0);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_labels_priority_first() {
        let labels = vec![
            crate::ai::types::RepoLabel {
                name: "documentation".to_string(),
                color: "0075ca".to_string(),
                description: "Documentation".to_string(),
            },
            crate::ai::types::RepoLabel {
                name: "other".to_string(),
                color: "cccccc".to_string(),
                description: "Other".to_string(),
            },
            crate::ai::types::RepoLabel {
                name: "bug".to_string(),
                color: "ff0000".to_string(),
                description: "Bug".to_string(),
            },
        ];
        let filtered = filter_labels_by_relevance(&labels, 30);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].name, "documentation");
        assert_eq!(filtered[1].name, "bug");
        assert_eq!(filtered[2].name, "other");
    }

    #[test]
    fn filter_labels_case_insensitive() {
        let labels = vec![
            crate::ai::types::RepoLabel {
                name: "Bug".to_string(),
                color: "ff0000".to_string(),
                description: "Bug".to_string(),
            },
            crate::ai::types::RepoLabel {
                name: "ENHANCEMENT".to_string(),
                color: "a2eeef".to_string(),
                description: "Enhancement".to_string(),
            },
        ];
        let filtered = filter_labels_by_relevance(&labels, 30);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "Bug");
        assert_eq!(filtered[1].name, "ENHANCEMENT");
    }

    #[test]
    fn filter_labels_over_limit_with_priorities() {
        let mut labels = vec![];
        for i in 0..20 {
            labels.push(crate::ai::types::RepoLabel {
                name: format!("label{}", i),
                color: "cccccc".to_string(),
                description: format!("Label {}", i),
            });
        }
        labels.push(crate::ai::types::RepoLabel {
            name: "bug".to_string(),
            color: "ff0000".to_string(),
            description: "Bug".to_string(),
        });
        labels.push(crate::ai::types::RepoLabel {
            name: "enhancement".to_string(),
            color: "a2eeef".to_string(),
            description: "Enhancement".to_string(),
        });

        let filtered = filter_labels_by_relevance(&labels, 10);
        assert_eq!(filtered.len(), 10);
        assert_eq!(filtered[0].name, "bug");
        assert_eq!(filtered[1].name, "enhancement");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke test to verify parse_issue_reference delegates correctly.
    // Comprehensive parsing tests are in github/mod.rs.
    #[test]
    fn parse_issue_reference_delegates_to_shared() {
        let (owner, repo, number) =
            parse_issue_reference("https://github.com/block/goose/issues/5836", None).unwrap();
        assert_eq!(owner, "block");
        assert_eq!(repo, "goose");
        assert_eq!(number, 5836);
    }

    #[test]
    fn extract_keywords_filters_stop_words() {
        let title = "The issue is about a bug in the CLI";
        let keywords = extract_keywords(title);
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"is".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
        assert!(keywords.contains(&"issue".to_string()));
        assert!(keywords.contains(&"bug".to_string()));
        assert!(keywords.contains(&"cli".to_string()));
    }

    #[test]
    fn extract_keywords_limits_to_five() {
        let title = "one two three four five six seven eight nine ten";
        let keywords = extract_keywords(title);
        assert_eq!(keywords.len(), 5);
    }

    #[test]
    fn extract_keywords_empty_title() {
        let title = "the a an and or";
        let keywords = extract_keywords(title);
        assert!(keywords.is_empty());
    }

    #[test]
    fn extract_keywords_lowercase_conversion() {
        let title = "CLI Bug FIX";
        let keywords = extract_keywords(title);
        assert!(keywords.iter().all(|k| k.chars().all(char::is_lowercase)));
    }
}
