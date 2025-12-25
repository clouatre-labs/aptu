// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally.

use anyhow::Result;
use tracing::{debug, instrument};

use super::types::PrReviewResult;

/// Review a pull request with AI assistance.
///
/// Fetches PR details and file diffs, then analyzes with AI.
///
/// # Arguments
///
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
#[instrument(skip_all, fields(reference = %reference))]
pub async fn run(reference: &str, repo_context: Option<&str>) -> Result<PrReviewResult> {
    // Create CLI token provider
    let provider = crate::provider::CliTokenProvider;

    // Call facade for PR review
    let (pr_details, review, ai_stats) =
        aptu_core::review_pr(&provider, reference, repo_context).await?;

    debug!(
        pr_number = pr_details.number,
        verdict = %review.verdict,
        "PR review complete"
    );

    Ok(PrReviewResult {
        pr_title: pr_details.title,
        pr_number: pr_details.number,
        pr_url: pr_details.url,
        review,
        ai_stats,
    })
}
