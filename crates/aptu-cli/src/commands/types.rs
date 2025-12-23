// SPDX-License-Identifier: Apache-2.0

//! Result types returned by command handlers.
//!
//! These types allow command handlers to return data instead of printing
//! directly, improving testability and separation of concerns.

use aptu_core::ai::types::TriageResponse;
use aptu_core::github::graphql::IssueNode;
use aptu_core::history::{Contribution, HistoryData};
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
#[derive(Debug, Clone)]
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
    /// Labels that were applied to the issue.
    pub applied_labels: Vec<String>,
    /// Milestone that was applied to the issue.
    pub applied_milestone: Option<String>,
    /// Warnings from applying labels/milestone.
    pub apply_warnings: Vec<String>,
}

/// Outcome of a single triage operation in a bulk operation.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum SingleTriageOutcome {
    /// Triage succeeded.
    #[allow(dead_code)]
    Success(Box<TriageResult>),
    /// Triage was skipped (e.g., already triaged).
    #[allow(dead_code)]
    Skipped(String),
    /// Triage failed with an error.
    #[allow(dead_code)]
    Failed(String),
}

/// Result from a bulk triage operation.
pub struct BulkTriageResult {
    /// Number of issues successfully triaged.
    pub succeeded: usize,
    /// Number of issues that failed.
    pub failed: usize,
    /// Number of issues that were skipped.
    pub skipped: usize,
    /// Individual outcomes for each issue.
    pub outcomes: Vec<(String, SingleTriageOutcome)>,
}

/// Result from the history command.
pub struct HistoryResult {
    /// List of contributions.
    pub contributions: Vec<Contribution>,
    /// Full history data for stats calculation.
    pub history_data: HistoryData,
}
