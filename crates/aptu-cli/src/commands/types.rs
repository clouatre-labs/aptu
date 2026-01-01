// SPDX-License-Identifier: Apache-2.0

//! Result types returned by command handlers.
//!
//! These types allow command handlers to return data instead of printing
//! directly, improving testability and separation of concerns.

use aptu_core::DiscoveredRepo;
use aptu_core::ai::types::TriageResponse;
use aptu_core::github::auth::TokenSource;
use aptu_core::github::graphql::IssueNode;
use aptu_core::history::{Contribution, HistoryData};
use aptu_core::repos::CuratedRepo;
use serde::Serialize;

/// Result from the auth status command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AuthStatusResult {
    /// Whether the user is authenticated.
    pub authenticated: bool,
    /// Authentication method (if authenticated).
    pub method: Option<TokenSource>,
    /// GitHub username (if authenticated and available).
    pub username: Option<String>,
}

/// Result from the repos command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ReposResult {
    /// List of curated repositories.
    pub repos: Vec<CuratedRepo>,
}

/// Result from the issues command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
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
    /// Whether the user is a maintainer (has write/maintain/admin permission).
    pub is_maintainer: bool,
}

/// Outcome of a single triage operation in a bulk operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
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

impl SingleTriageOutcome {
    /// Extract `TriageResult` if this is a Success outcome.
    pub fn as_triage_result(&self) -> Option<&TriageResult> {
        match self {
            SingleTriageOutcome::Success(result) => Some(result),
            _ => None,
        }
    }
}

/// Result from a bulk triage operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
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

impl BulkTriageResult {
    /// Check if any outcomes are dry-run operations.
    pub fn has_dry_run(&self) -> bool {
        self.outcomes.iter().any(|(_, outcome)| {
            outcome
                .as_triage_result()
                .is_some_and(|result| result.dry_run)
        })
    }
}

/// Result from the history command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HistoryResult {
    /// List of contributions.
    pub contributions: Vec<Contribution>,
    /// Full history data for stats calculation.
    pub history_data: HistoryData,
}

/// Result from the create command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateResult {
    /// URL of the created issue.
    pub issue_url: String,
    /// Issue number.
    pub issue_number: u64,
    /// Issue title that was created.
    pub title: String,
    /// Issue body that was created.
    pub body: String,
    /// AI-suggested labels for the issue.
    pub suggested_labels: Vec<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
}

/// Result from the PR review command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PrReviewResult {
    /// PR title.
    pub pr_title: String,
    /// PR number.
    pub pr_number: u64,
    /// PR URL.
    pub pr_url: String,
    /// AI review response.
    pub review: aptu_core::ai::types::PrReviewResponse,
    /// AI usage statistics.
    pub ai_stats: aptu_core::history::AiStats,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// PR labels.
    pub labels: Vec<String>,
}

/// Result from the PR label command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PrLabelResult {
    /// PR number.
    pub pr_number: u64,
    /// PR title.
    pub pr_title: String,
    /// PR URL.
    pub pr_url: String,
    /// Labels extracted and applied.
    pub labels: Vec<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
}

/// Result from the discover command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DiscoverResult {
    /// List of discovered repositories.
    pub repos: Vec<DiscoveredRepo>,
}
