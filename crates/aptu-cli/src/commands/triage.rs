// SPDX-License-Identifier: Apache-2.0

//! Triage an issue with AI assistance command.
//!
//! Fetches a GitHub issue, analyzes it with AI, and optionally posts
//! a comment to GitHub. Split into `fetch()` and `analyze()` for proper
//! confirmation flow (render issue before asking).

use anyhow::Result;
use aptu_core::ai::AiResponse;
use aptu_core::{IssueDetails, TriageResponse, history::AiStats};
use tracing::{debug, info, instrument};

use crate::provider::CliTokenProvider;

/// Intermediate result from analysis (before posting decision).
pub struct AnalyzeResult {
    /// Issue details (title, body, labels, comments).
    pub issue_details: IssueDetails,
    /// AI triage analysis.
    pub triage: TriageResponse,
    /// AI usage statistics.
    #[allow(dead_code)] // Used for future features (history tracking)
    pub ai_stats: AiStats,
}

/// Fetch an issue from GitHub.
///
/// Parses the issue reference, checks authentication, and fetches issue details.
/// Also searches for related issues to provide context to the AI.
/// Does not perform AI analysis.
///
/// # Arguments
///
/// * `reference` - Issue reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
#[instrument(skip_all, fields(reference = %reference))]
pub async fn fetch(reference: &str, repo_context: Option<&str>) -> Result<IssueDetails> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade to fetch issue
    let issue_details =
        aptu_core::fetch_issue_for_triage(&provider, reference, repo_context).await?;

    debug!(
        issue_number = issue_details.number,
        "Issue fetched successfully"
    );
    Ok(issue_details)
}

/// Analyze an issue with AI assistance.
///
/// Takes fetched issue details and runs AI analysis via the facade layer.
/// Returns both triage response and AI usage statistics.
/// Does not post anything.
///
/// # Arguments
///
/// * `issue_details` - Fetched issue details from `fetch()`
#[instrument(skip_all, fields(issue_number = issue_details.number))]
pub async fn analyze(
    issue_details: &IssueDetails,
    ai_config: &aptu_core::AiConfig,
) -> Result<AiResponse> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let ai_response = aptu_core::analyze_issue(&provider, issue_details, ai_config).await?;

    debug!("Issue analyzed successfully");
    Ok(ai_response)
}

/// Post a triage comment to GitHub.
#[instrument(skip_all, fields(issue_number = analyze_result.issue_details.number))]
pub async fn post(analyze_result: &AnalyzeResult) -> Result<String> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade to post comment
    let comment_url = aptu_core::post_triage_comment(
        &provider,
        &analyze_result.issue_details,
        &analyze_result.triage,
    )
    .await?;

    info!(comment_url = %comment_url, "Triage comment posted");
    debug!(
        "Triage complete for issue #{}",
        analyze_result.issue_details.number
    );

    Ok(comment_url)
}

/// Apply AI-suggested labels and milestone to an issue.
///
/// Labels are applied additively: existing labels are preserved and AI-suggested labels are merged in.
/// Priority labels (p1/p2/p3) defer to existing human judgment.
/// Milestones are only set if the issue doesn't already have one.
/// Validates suggestions against available labels and milestones before applying.
/// Returns what was applied and any warnings.
///
/// # Arguments
///
/// * `issue_details` - Issue details including available labels and milestones
/// * `triage` - AI triage response with suggestions
#[instrument(skip_all, fields(owner = %issue_details.owner, repo = %issue_details.repo, number = issue_details.number))]
pub async fn apply(
    issue_details: &IssueDetails,
    triage: &TriageResponse,
) -> Result<aptu_core::github::issues::ApplyResult> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    debug!("Applying labels and milestone to issue");

    // Call facade to apply labels
    let result = aptu_core::apply_triage_labels(&provider, issue_details, triage).await?;

    info!(
        labels = ?result.applied_labels,
        milestone = ?result.applied_milestone,
        warnings = ?result.warnings,
        "Labels and milestone applied"
    );

    Ok(result)
}
