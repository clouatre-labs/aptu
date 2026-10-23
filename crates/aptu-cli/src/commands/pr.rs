// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.
//! Split into `fetch()` and `analyze()` for proper display flow (show PR details
//! before AI spinner).

use anyhow::Result;
use console::style;

use super::ReviewOptions;
use super::types::BulkPrReviewResult;
use aptu_core::ai::types::PrReviewComment;
use aptu_core::metrics::AiStats;
use aptu_core::{
    PrDetails, PrReviewResponse, render_pr_review_comment_body, render_pr_review_review_body,
};
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
) -> Result<(
    PrReviewResponse,
    aptu_core::metrics::AiStats,
    aptu_core::metrics::ReviewContextRecord,
)> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let (review, ai_stats, context_record) =
        aptu_core::analyze_pr(&provider, pr_details, ai_config).await?;

    debug!("PR analyzed successfully");
    Ok((review, ai_stats, context_record))
}

/// Format the header line for a single inline review comment.
///
/// Returns `"<file>[:<line>]  [<SEVERITY>]"`.
pub(crate) fn format_comment_header(comment: &PrReviewComment) -> String {
    let line_part = comment.line.map_or_else(String::new, |l| format!(":{l}"));
    let severity = comment.severity.as_str().to_uppercase();
    format!("{}{}  [{}]", comment.file, line_part, severity)
}

/// Post a PR review to GitHub.
#[instrument(skip_all, fields(pr_number = analyze_result.pr_details.number))]
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub async fn post(
    analyze_result: &AnalyzeResult,
    reference: &str,
    repo_context: Option<&str>,
    event: aptu_core::ReviewEvent,
    dry_run: bool,
    skip_confirm: bool,
    verbose: bool,
    no_dedup_summary: bool,
) -> Result<()> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // The PR review body is the single summary surface: it carries the marker,
    // verdict, summary, Concerns, and Files Changed table (#1695).
    let review_body = render_pr_review_review_body(
        &analyze_result.review,
        &analyze_result.pr_details.files,
        &analyze_result.pr_details.head_sha,
    );

    if dry_run {
        debug!("Dry-run mode: skipping post");
        eprintln!(
            "Dry-run: Would post {} review to PR #{}",
            event, analyze_result.pr_details.number
        );
        eprintln!("Review body:\n{review_body}");
        if verbose && !analyze_result.review.comments.is_empty() {
            eprintln!(
                "\nInline comments ({}):",
                analyze_result.review.comments.len()
            );
            for (i, comment) in analyze_result.review.comments.iter().enumerate() {
                eprintln!("  [{}] {}", i + 1, format_comment_header(comment));
                let body = render_pr_review_comment_body(comment);
                let indented = body
                    .lines()
                    .map(|l| format!("      {l}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                eprintln!("{indented}\n");
            }
        }
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

        // Post the review with inline comments and head SHA.
        let outcome = aptu_core::post_pr_review(
            &provider,
            reference,
            repo_context,
            &review_body,
            event,
            &analyze_result.review.comments,
            &analyze_result.pr_details.head_sha,
            &analyze_result.pr_details.review_comments,
            !no_dedup_summary,
            &aptu_core::diff_hash(&analyze_result.pr_details.files),
        )
        .await?;

        if outcome.is_skipped() {
            eprintln!("No write access to this repository - review not posted");
            return Ok(());
        }

        let outcome = outcome.applied().expect("outcome checked for skip above");

        match outcome.summary {
            aptu_core::SummaryPostOutcome::Skipped => {
                eprintln!(
                    "Review already up to date for head SHA {}; nothing posted",
                    analyze_result.pr_details.head_sha
                );
            }
            // Unreachable in practice: submitted reviews are immutable, so a
            // changed head SHA posts a NEW review (reported as Posted). The arm
            // stays for API compatibility with SummaryPostOutcome (#1695).
            #[allow(deprecated)] // SummaryPostOutcome::Updated is deprecated; see #1695
            aptu_core::SummaryPostOutcome::Updated => {
                info!(review_id = outcome.review_id, "Review posted successfully");
                eprintln!("Review posted successfully (ID: {})", outcome.review_id);
            }
            aptu_core::SummaryPostOutcome::Posted => {
                info!(review_id = outcome.review_id, "Review posted successfully");
                eprintln!("Review posted successfully (ID: {})", outcome.review_id);
            }
        }
        if !outcome.failed_comments.is_empty() {
            eprintln!(
                "Warning: {} inline comment(s) failed to post: {}",
                outcome.failed_comments.len(),
                outcome.failed_comments.join(", ")
            );
        }
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
) -> Result<(PrLabelResult, AiStats)> {
    // Create CLI token provider
    let provider = crate::provider::CliTokenProvider;

    // Call facade for PR label
    let outcome =
        aptu_core::label_pr(&provider, reference, repo_context, dry_run, ai_config).await?;

    if outcome.is_skipped() {
        eprintln!("No write access to this repository - labels not applied");
        return Ok((PrLabelResult::empty(dry_run), AiStats::default()));
    }

    let (pr_number, pr_title, pr_url, labels, ai_stats) =
        outcome.applied().expect("outcome checked for skip above");

    Ok((
        PrLabelResult {
            pr_number: *pr_number,
            pr_title: pr_title.clone(),
            pr_url: pr_url.clone(),
            labels: labels.clone(),
            dry_run,
        },
        ai_stats.clone(),
    ))
}

/// Run the PR review command.
#[allow(clippy::too_many_lines)]
#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
pub async fn run_review(
    references: Vec<String>,
    repo: Option<String>,
    comment: bool,
    approve: bool,
    request_changes: bool,
    dry_run: bool,
    _no_apply: bool,
    no_comment: bool,
    force: bool,
    instructions_file: Option<std::path::PathBuf>,
    ctx: crate::cli::OutputContext,
    config: &aptu_core::AppConfig,
    inferred_repo: Option<String>,
    no_dedup_summary: bool,
) -> Result<()> {
    use crate::cli::OutputFormat;

    let repo_context = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(config.user.default_repo.as_deref());

    // Determine review type from flags
    let review_type = if comment {
        Some(aptu_core::ReviewEvent::Comment)
    } else if approve {
        Some(aptu_core::ReviewEvent::Approve)
    } else if request_changes {
        Some(aptu_core::ReviewEvent::RequestChanges)
    } else {
        None
    };

    if references.is_empty() {
        if matches!(ctx.format, OutputFormat::Text) {
            println!("{}", style("No PRs to review.").yellow());
        }
        return Ok(());
    }

    // Bulk PR review using core processor
    let items: Vec<(String, ())> = references.iter().map(|r| (r.clone(), ())).collect();

    let ctx_for_processor = ctx.clone();
    let ctx_for_progress = ctx.clone();
    let repo_context_owned = repo_context.map(std::string::ToString::to_string);
    let mut config_clone = config.clone();
    let instructions_file_str = instructions_file.map(|p| p.to_string_lossy().into_owned());

    // Override instructions_file in config if provided via CLI
    if let Some(path) = &instructions_file_str {
        config_clone.review.instructions_file = Some(path.clone());
    }

    let core_result = aptu_core::process_bulk(
        items,
        move |(pr_ref, ())| {
            let ctx = ctx_for_processor.clone();
            let repo_context = repo_context_owned.clone();
            let config = config_clone.clone();
            async move {
                let outcome = super::review_single_pr(
                    &pr_ref,
                    repo_context.as_deref(),
                    review_type,
                    ReviewOptions {
                        dry_run,
                        yes: !ctx.is_interactive() || force,
                        no_comment,
                        no_dedup_summary,
                    },
                    &ctx,
                    &config,
                )
                .await?;
                Ok(match outcome {
                    Some(result) => aptu_core::BulkItem::Done(result),
                    None => aptu_core::BulkItem::Skipped("Skipped".to_string()),
                })
            }
        },
        move |current, total, action| {
            crate::output::common::show_progress(&ctx_for_progress, current, total, action);
        },
    )
    .await;

    // Convert core BulkResult to CLI BulkPrReviewResult
    let mut bulk_result = BulkPrReviewResult {
        succeeded: core_result.succeeded,
        failed: core_result.failed,
        skipped: core_result.skipped,
        outcomes: Vec::new(),
    };

    for (pr_ref, outcome) in core_result.outcomes {
        let cli_outcome = super::report_outcome(outcome, &ctx);
        bulk_result.outcomes.push((pr_ref, cli_outcome));
    }

    // Render bulk summary (only for multiple PRs)
    if references.len() > 1 {
        crate::output::render(&bulk_result, &ctx)?;
    }

    Ok(())
}

/// Run the PR label command (spinner, metrics, rendering wrapper).
pub async fn run_label_command(
    reference: String,
    repo: Option<String>,
    dry_run: bool,
    ctx: crate::cli::OutputContext,
    config: &aptu_core::AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    use crate::commands::common::maybe_spinner;

    let repo_context = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(config.user.default_repo.as_deref());

    let spinner = maybe_spinner(&ctx, "Fetching PR and extracting labels...");
    let (result, ai_stats) = run_label(&reference, repo_context, dry_run, &config.ai).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }
    aptu_core::metrics::append_jsonl(&ai_stats);
    crate::output::render(&result, &ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptu_core::ai::types::{CommentSeverity, PrReviewComment};

    #[test]
    fn test_format_comment_header_with_line() {
        let comment = PrReviewComment {
            file: "src/main.rs".to_string(),
            line: Some(42),
            comment: "Test comment".to_string(),
            severity: CommentSeverity::Warning,
            confidence: 1.0,
            suggested_code: None,
        };
        let header = format_comment_header(&comment);
        assert_eq!(header, "src/main.rs:42  [WARNING]");
    }

    #[test]
    fn test_format_comment_header_no_line() {
        let comment = PrReviewComment {
            file: "src/lib.rs".to_string(),
            line: None,
            comment: "Test comment".to_string(),
            severity: CommentSeverity::Info,
            confidence: 1.0,
            suggested_code: None,
        };
        let header = format_comment_header(&comment);
        assert_eq!(header, "src/lib.rs  [INFO]");
    }
}
