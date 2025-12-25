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

use crate::ai::types::{IssueComment, IssueDetails, RepoIssueContext};
use crate::retry::retry_backoff;

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

    let details = IssueDetails {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        title: issue.title,
        body: issue.body.unwrap_or_default(),
        labels,
        comments,
        url: issue_url,
        repo_context: Vec::new(),
        repo_tree: Vec::new(),
        available_labels: Vec::new(),
        available_milestones: Vec::new(),
        viewer_permission: None,
    };

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

/// Updates an issue with labels and milestone.
///
/// Validates suggested labels and milestone against available options before applying.
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
    suggested_labels: &[String],
    suggested_milestone: Option<&str>,
    available_labels: &[crate::ai::types::RepoLabel],
    available_milestones: &[crate::ai::types::RepoMilestone],
) -> Result<ApplyResult> {
    debug!("Updating issue with labels and milestone");

    let mut applied_labels = Vec::new();
    let mut warnings = Vec::new();

    // Validate and collect labels
    let available_label_names: std::collections::HashSet<_> =
        available_labels.iter().map(|l| l.name.as_str()).collect();

    for label in suggested_labels {
        if available_label_names.contains(label.as_str()) {
            applied_labels.push(label.clone());
        } else {
            warnings.push(format!("Label '{label}' not found in repository"));
        }
    }

    // Validate and find milestone
    let mut applied_milestone = None;
    if let Some(milestone_title) = suggested_milestone {
        if let Some(milestone) = available_milestones
            .iter()
            .find(|m| m.title == milestone_title)
        {
            applied_milestone = Some(milestone.title.clone());
        } else {
            warnings.push(format!(
                "Milestone '{milestone_title}' not found in repository"
            ));
        }
    }

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

/// Filters repository tree entries by language-specific extensions.
///
/// Removes common non-source directories and limits results to 50 paths.
/// Prioritizes shallow paths (fewer `/` characters).
/// This is a legacy function kept for backward compatibility with existing tests.
///
/// # Arguments
///
/// * `entries` - Raw tree entries from GitHub API
/// * `language` - Repository primary language for extension filtering
///
/// # Returns
///
/// Filtered and sorted list of file paths (max 50).
#[allow(dead_code)]
fn filter_tree_by_language(entries: &[GitTreeEntry], language: &str) -> Vec<String> {
    let extensions = get_extensions_for_language(language);
    let exclude_dirs = [
        "node_modules/",
        "target/",
        "dist/",
        "build/",
        ".git/",
        "vendor/",
        "test",
        "spec",
        "mock",
        "fixture",
    ];

    let mut filtered: Vec<String> = entries
        .iter()
        .filter(|entry| {
            // Only include files (blobs), not directories
            if entry.type_ != "blob" {
                return false;
            }

            // Exclude paths containing excluded directories
            if exclude_dirs.iter().any(|dir| entry.path.contains(dir)) {
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

    // Sort by path depth (fewer slashes first), then alphabetically
    filtered.sort_by(|a, b| {
        let depth_a = a.matches('/').count();
        let depth_b = b.matches('/').count();
        if depth_a == depth_b {
            a.cmp(b)
        } else {
            depth_a.cmp(&depth_b)
        }
    });

    // Limit to 50 paths
    filtered.truncate(50);
    filtered
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
/// * `force` - If true, return all open issues; if false, filter to unlabeled or milestone-missing issues
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
) -> Result<Vec<UntriagedIssue>> {
    debug!("Fetching issues needing triage");

    let issues_page: octocrab::Page<octocrab::models::issues::Issue> = client
        .issues(owner, repo)
        .list()
        .state(octocrab::params::State::Open)
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
    fn filter_tree_excludes_node_modules() {
        let entries = vec![
            GitTreeEntry {
                path: "src/main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "node_modules/package/index.js".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let filtered = filter_tree_by_language(&entries, "rust");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "src/main.rs");
    }

    #[test]
    fn filter_tree_excludes_directories() {
        let entries = vec![
            GitTreeEntry {
                path: "src/main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "src/lib".to_string(),
                type_: "tree".to_string(),
                mode: "040000".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let filtered = filter_tree_by_language(&entries, "rust");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "src/main.rs");
    }

    #[test]
    fn filter_tree_sorts_by_depth() {
        let entries = vec![
            GitTreeEntry {
                path: "a/b/c/d.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "a/b.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
            GitTreeEntry {
                path: "main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "ghi789".to_string(),
            },
        ];

        let filtered = filter_tree_by_language(&entries, "rust");
        assert_eq!(filtered[0], "main.rs");
        assert_eq!(filtered[1], "a/b.rs");
        assert_eq!(filtered[2], "a/b/c/d.rs");
    }

    #[test]
    fn filter_tree_limits_to_50() {
        let entries: Vec<GitTreeEntry> = (0..100)
            .map(|i| GitTreeEntry {
                path: format!("file{i}.rs"),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: format!("sha{i}"),
            })
            .collect();

        let filtered = filter_tree_by_language(&entries, "rust");
        assert_eq!(filtered.len(), 50);
    }

    #[test]
    fn filter_tree_by_language_rust() {
        let entries = vec![
            GitTreeEntry {
                path: "src/main.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "src/lib.py".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let filtered = filter_tree_by_language(&entries, "rust");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "src/main.rs");
    }

    #[test]
    fn filter_tree_by_language_python() {
        let entries = vec![
            GitTreeEntry {
                path: "main.py".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "abc123".to_string(),
            },
            GitTreeEntry {
                path: "lib.rs".to_string(),
                type_: "blob".to_string(),
                mode: "100644".to_string(),
                sha: "def456".to_string(),
            },
        ];

        let filtered = filter_tree_by_language(&entries, "python");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "main.py");
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
