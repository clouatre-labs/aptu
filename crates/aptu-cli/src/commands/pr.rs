// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.
//! Split into `fetch()` and `analyze()` for proper display flow (show PR details
//! before AI spinner).

use anyhow::Result;
use aptu_core::ai::types::PrReviewComment;
use aptu_core::{
    PrDetails, PrReviewResponse, history::AiStats, render_pr_review_comment_body,
    render_pr_review_markdown,
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
) -> Result<(PrReviewResponse, aptu_core::history::AiStats)> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let (review, ai_stats) = aptu_core::analyze_pr(&provider, pr_details, ai_config).await?;

    debug!("PR analyzed successfully");
    Ok((review, ai_stats))
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
pub async fn post(
    analyze_result: &AnalyzeResult,
    reference: &str,
    repo_context: Option<&str>,
    event: aptu_core::ReviewEvent,
    dry_run: bool,
    skip_confirm: bool,
    verbose: bool,
) -> Result<()> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    let review_body = render_pr_review_markdown(
        &analyze_result.review,
        analyze_result.pr_details.files.len(),
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
                for line in body.lines() {
                    eprintln!("      {line}");
                }
                eprintln!();
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
        let review_id = aptu_core::post_pr_review(
            &provider,
            reference,
            repo_context,
            &review_body,
            event,
            &analyze_result.review.comments,
            &analyze_result.pr_details.head_sha,
        )
        .await?;

        info!(review_id = review_id, "Review posted successfully");
        eprintln!("Review posted successfully (ID: {review_id})");
    }

    Ok(())
}

/// Create a pull request on GitHub.
///
/// Resolves the head branch from git if not provided, resolves repo from context,
/// and calls the core facade to create the PR.
///
/// # Arguments
///
/// * `repo` - Optional repository override (owner/repo)
/// * `inferred_repo` - Repository inferred from git remote
/// * `default_repo` - Default repository from config
/// * `title` - PR title
/// * `body` - Optional PR body
/// * `branch` - Optional head branch (defaults to current git branch)
/// * `base` - Base branch to merge into
#[instrument(skip_all)]
pub async fn run_pr_create(
    repo: Option<String>,
    inferred_repo: Option<String>,
    default_repo: Option<String>,
    title: String,
    body: Option<String>,
    branch: Option<String>,
    base: String,
) -> anyhow::Result<aptu_core::PrCreateResult> {
    use aptu_core::github::parse_owner_repo;

    // Resolve repo
    let resolved_repo = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(default_repo.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No repository specified. Use --repo or run inside a git repo with a GitHub remote."
            )
        })?;
    let (owner, repo_name) = parse_owner_repo(resolved_repo)?;

    // Resolve head branch
    let head = if let Some(b) = branch {
        b
    } else {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run git: {e}"))?;
        if !output.status.success() {
            anyhow::bail!("Failed to determine current git branch. Use --branch to specify.");
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    let provider = CliTokenProvider;

    aptu_core::create_pr(
        &provider,
        &owner,
        &repo_name,
        &title,
        &base,
        &head,
        body.as_deref(),
    )
    .await
    .map_err(Into::into)
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
            suggested_code: None,
        };
        let header = format_comment_header(&comment);
        assert_eq!(header, "src/lib.rs  [INFO]");
    }
}
