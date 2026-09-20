// SPDX-License-Identifier: Apache-2.0

//! Result types returned by command handlers.
//!
//! These types allow command handlers to return data instead of printing
//! directly, improving testability and separation of concerns.

use aptu_core::ai::types::TriageResponse;
use aptu_core::github::auth::TokenSource;
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
    /// AI provider authentication method (if configured).
    pub ai_provider: Option<String>,
    /// AI provider auth method: "api-key" or "oauth".
    pub ai_auth_method: Option<String>,
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
    /// AI usage statistics.
    pub ai_stats: aptu_core::history::AiStats,
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
    Success(Box<TriageResult>),
    /// Triage was skipped (e.g., already triaged).
    Skipped(String),
    /// Triage failed with an error.
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

/// Conversion from a core bulk outcome to a CLI single outcome.
///
/// Implemented by the per-command single-outcome enums so `report_outcome`
/// can map `aptu_core::BulkOutcome` values uniformly.
pub trait OutcomeInfo: Sized {
    /// Success payload carried by the core bulk outcome.
    type Inner;

    /// Wrap a success payload.
    fn from_success(inner: Self::Inner) -> Self;

    /// Wrap a skip message.
    fn from_skipped(msg: String) -> Self;

    /// Wrap a failure error message.
    fn from_failed(err: String) -> Self;
}

impl OutcomeInfo for SingleTriageOutcome {
    type Inner = TriageResult;

    fn from_success(inner: Self::Inner) -> Self {
        SingleTriageOutcome::Success(Box::new(inner))
    }

    fn from_skipped(msg: String) -> Self {
        SingleTriageOutcome::Skipped(msg)
    }

    fn from_failed(err: String) -> Self {
        SingleTriageOutcome::Failed(err)
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
    /// Review verdict (`approve`, `request_changes`, `comment`).
    pub verdict: String,
    /// AI usage statistics.
    pub ai_stats: aptu_core::history::AiStats,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// PR labels.
    pub labels: Vec<String>,
    /// Security findings from scanning (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_findings: Option<Vec<aptu_core::Finding>>,
    /// Total number of files in the PR.
    pub files_total: usize,
    /// Number of files whose patch was included in the review prompt (after budget drops).
    pub files_with_patch: usize,
}

/// Outcome of a single PR review operation in a bulk operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum SinglePrReviewOutcome {
    /// PR review succeeded.
    Success(Box<PrReviewResult>),
    /// PR review was skipped.
    Skipped(String),
    /// PR review failed with an error.
    Failed(String),
}

impl SinglePrReviewOutcome {
    /// Extract `PrReviewResult` if this is a Success outcome.
    pub fn as_pr_review_result(&self) -> Option<&PrReviewResult> {
        match self {
            SinglePrReviewOutcome::Success(result) => Some(result),
            _ => None,
        }
    }
}

impl OutcomeInfo for SinglePrReviewOutcome {
    type Inner = PrReviewResult;

    fn from_success(inner: Self::Inner) -> Self {
        SinglePrReviewOutcome::Success(Box::new(inner))
    }

    fn from_skipped(msg: String) -> Self {
        SinglePrReviewOutcome::Skipped(msg)
    }

    fn from_failed(err: String) -> Self {
        SinglePrReviewOutcome::Failed(err)
    }
}

/// Result from a bulk PR review operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BulkPrReviewResult {
    /// Number of PRs successfully reviewed.
    pub succeeded: usize,
    /// Number of PRs that failed.
    pub failed: usize,
    /// Number of PRs that were skipped.
    pub skipped: usize,
    /// Individual outcomes for each PR.
    pub outcomes: Vec<(String, SinglePrReviewOutcome)>,
}

impl BulkPrReviewResult {
    /// Check if any outcomes are dry-run operations.
    pub fn has_dry_run(&self) -> bool {
        self.outcomes
            .iter()
            .any(|(_, outcome)| outcome.as_pr_review_result().is_some_and(|r| r.dry_run))
    }
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

impl PrLabelResult {
    /// Sentinel result for the skip branch: `pr_number` 0, empty title/url,
    /// and no labels, echoing `dry_run`.
    pub fn empty(dry_run: bool) -> Self {
        Self {
            pr_number: 0,
            pr_title: String::new(),
            pr_url: String::new(),
            labels: Vec::new(),
            dry_run,
        }
    }
}

/// Result from auth login or logout actions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AuthActionResult {
    /// Action performed (e.g., "login", "logout").
    pub action: String,
    /// Human-readable message describing the outcome.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::PrLabelResult;

    #[test]
    fn empty_returns_sentinel_values() {
        let result = PrLabelResult::empty(true);
        assert_eq!(result.pr_number, 0);
        assert_eq!(result.pr_title, "");
        assert_eq!(result.pr_url, "");
        assert!(result.labels.is_empty());
        assert!(result.dry_run);
    }
}
