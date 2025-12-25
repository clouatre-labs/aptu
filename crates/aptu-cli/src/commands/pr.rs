// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.

use anyhow::Result;
use tracing::{debug, instrument};

use super::types::PrReviewResult;

/// Review a pull request with AI assistance.
///
/// Fetches PR details and file diffs, then analyzes with AI.
/// Optionally posts the review to GitHub if a review type flag is provided.
///
/// # Arguments
///
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `review_type` - Optional review type (comment, approve, or `request_changes`)
/// * `dry_run` - If true, preview without posting
/// * `skip_confirm` - If true, skip confirmation prompt
#[instrument(skip_all, fields(reference = %reference, review_type = ?review_type))]
pub async fn run(
    reference: &str,
    repo_context: Option<&str>,
    review_type: Option<aptu_core::ReviewEvent>,
    dry_run: bool,
    skip_confirm: bool,
) -> Result<PrReviewResult> {
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

    // If review type is specified, handle posting workflow
    if let Some(event) = review_type {
        let review_body = format!(
            "## AI Review\n\n{}\n\n**Verdict:** {}\n\n",
            review.summary, review.verdict
        );

        if dry_run {
            debug!("Dry-run mode: skipping post");
            eprintln!(
                "Dry-run: Would post {} review to PR #{}",
                event, pr_details.number
            );
            eprintln!("Review body:\n{review_body}");
        } else {
            // Confirm before posting unless --yes flag is set
            if !skip_confirm {
                eprintln!(
                    "About to post {} review to PR #{}",
                    event, pr_details.number
                );
                eprintln!("Continue? (y/n) ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    debug!("User cancelled review posting");
                    return Ok(PrReviewResult {
                        pr_title: pr_details.title,
                        pr_number: pr_details.number,
                        pr_url: pr_details.url,
                        review,
                        ai_stats,
                    });
                }
            }

            // Post the review
            let review_id =
                aptu_core::post_pr_review(&provider, reference, repo_context, &review_body, event)
                    .await?;

            debug!(review_id = review_id, "Review posted successfully");
            eprintln!("Review posted successfully (ID: {review_id})");
        }
    }

    Ok(PrReviewResult {
        pr_title: pr_details.title,
        pr_number: pr_details.number,
        pr_url: pr_details.url,
        review,
        ai_stats,
    })
}
