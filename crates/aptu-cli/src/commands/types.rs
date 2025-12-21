// SPDX-License-Identifier: Apache-2.0

//! Result types returned by command handlers.
//!
//! These types allow command handlers to return data instead of printing
//! directly, improving testability and separation of concerns.

use aptu_core::ai::types::TriageResponse;
use aptu_core::github::graphql::IssueNode;
use aptu_core::history::Contribution;
use aptu_core::repos::CuratedRepo;

/// Result from the repos command.
pub struct ReposResult {
    /// List of curated repositories.
    pub repos: &'static [CuratedRepo],
}

/// Result from the issues command.
pub struct IssuesResult {
    /// Issues grouped by repository name.
    pub issues_by_repo: Vec<(String, Vec<IssueNode>)>,
    /// Total issue count across all repositories.
    pub total_count: usize,
    /// Repository filter that was applied (if any).
    pub repo_filter: Option<String>,
    /// Whether no repos matched the filter.
    pub no_repos_matched: bool,
}

/// Result from the triage command.
pub struct TriageResult {
    /// Issue title (for display).
    pub issue_title: String,
    /// Issue number.
    pub issue_number: u64,
    /// AI triage analysis.
    pub triage: TriageResponse,
    /// URL of posted comment (if posted).
    pub comment_url: Option<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Whether the user declined to post.
    pub user_declined: bool,
}

/// Result from the history command.
pub struct HistoryResult {
    /// List of contributions.
    pub contributions: Vec<Contribution>,
}
