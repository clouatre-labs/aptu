// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.
//! Split into `fetch()` and `analyze()` for proper display flow (show PR details
//! before AI spinner).

use anyhow::Result;
use aptu_core::{PrDetails, PrReviewResponse, history::AiStats};
use tracing::{debug, info, instrument};

use super::types::PrLabelResult;
use crate::provider::CliTokenProvider;

/// Intermediate result from analysis (before posting decision).
pub struct AnalyzeResult {
    /// PR details (title, body, labels, files).
    pub pr_details: PrDetails,
    /// AI review analysis.
    pub review: PrReviewResponse,
    /// AI usage statistics.
    #[allow(dead_code)]
    pub ai_stats: AiStats,
}

/// Fetch a pull request from GitHub.
///
/// Parses the PR reference, checks authentication, and fetches PR details
/// including file diffs. Does not perform AI analysis.
///
/// # Arguments
///
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
#[instrument(skip_all, fields(reference = %reference))]
pub async fn fetch(reference: &str, repo_context: Option<&str>) -> Result<PrDetails> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade to fetch PR
    let pr_details = aptu_core::fetch_pr_for_review(&provider, reference, repo_context).await?;

    debug!(pr_number = pr_details.number, "PR fetched successfully");
    Ok(pr_details)
}

/// Analyze a pull request with AI assistance.
///
/// Takes fetched PR details and runs AI analysis via the facade layer.
/// Returns both review response and AI usage statistics.
/// Does not post anything.
///
/// # Arguments
///
/// * `pr_details` - Fetched PR details from `fetch()`
/// * `ai_config` - AI configuration
#[instrument(skip_all, fields(pr_number = pr_details.number))]
pub async fn analyze(
    pr_details: &PrDetails,
    ai_config: &aptu_core::AiConfig,
) -> Result<(PrReviewResponse, aptu_core::history::AiStats)> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let (review, ai_stats) = aptu_core::analyze_pr(&provider, pr_details, ai_config).await?;

    debug!("PR analyzed successfully");
    Ok((review, ai_stats))
}

/// Post a PR review to GitHub.
#[instrument(skip_all, fields(pr_number = analyze_result.pr_details.number))]
pub async fn post(
    analyze_result: &AnalyzeResult,
    reference: &str,
    repo_context: Option<&str>,
    event: aptu_core::ReviewEvent,
    dry_run: bool,
    skip_confirm: bool,
) -> Result<()> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    let review_body = format!(
        "## Aptu Review\n\n{}\n\n**Verdict:** {}\n\n",
        analyze_result.review.summary, analyze_result.review.verdict
    );

    if dry_run {
        debug!("Dry-run mode: skipping post");
        eprintln!(
            "Dry-run: Would post {} review to PR #{}",
            event, analyze_result.pr_details.number
        );
        eprintln!("Review body:\n{review_body}");
    } else {
        // Confirm before posting unless --yes flag is set
        if !skip_confirm {
            eprintln!(
                "About to post {} review to PR #{}",
                event, analyze_result.pr_details.number
            );
            eprintln!("Continue? (y/n) ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                debug!("User cancelled review posting");
                return Ok(());
            }
        }

        // Post the review
        let review_id =
            aptu_core::post_pr_review(&provider, reference, repo_context, &review_body, event)
                .await?;

        info!(review_id = review_id, "Review posted successfully");
        eprintln!("Review posted successfully (ID: {review_id})");
    }

    Ok(())
}

/// Auto-label a pull request based on conventional commit prefix and file paths.
///
/// Fetches PR details, extracts labels from title and changed files,
/// and applies them to the PR. Optionally previews without applying.
///
/// # Arguments
///
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `dry_run` - If true, preview labels without applying
/// * `ai_config` - AI configuration for fallback label suggestion
#[instrument(skip_all, fields(reference = %reference))]
pub async fn run_label(
    reference: &str,
    repo_context: Option<&str>,
    dry_run: bool,
    ai_config: &aptu_core::AiConfig,
) -> Result<PrLabelResult> {
    // Create CLI token provider
    let provider = crate::provider::CliTokenProvider;

    // Call facade for PR label
    let (pr_number, pr_title, pr_url, labels) =
        aptu_core::label_pr(&provider, reference, repo_context, dry_run, ai_config).await?;

    Ok(PrLabelResult {
        pr_number,
        pr_title,
        pr_url,
        labels,
        dry_run,
    })
}
