// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.
//! Split into `fetch()` and `analyze()` for proper display flow (show PR details
//! before AI spinner).

use anyhow::{Context, Result};
use console::style;

use super::ReviewOptions;
use super::types::BulkPrReviewResult;
use aptu_core::ai::types::PrReviewComment;
use aptu_core::history::AiStats;
use aptu_core::{
    PrDetails, PrReviewResponse, render_pr_review_comment_body, render_pr_review_markdown,
    render_pr_review_review_body,
};
use tracing::{debug, info, instrument, warn};

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
    repo_path: Option<String>,
) -> Result<(
    PrReviewResponse,
    aptu_core::history::AiStats,
    aptu_core::metrics::ReviewContextRecord,
)> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let (review, ai_stats, context_record) =
        aptu_core::analyze_pr(&provider, pr_details, ai_config, repo_path).await?;

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

    // Summary lives ONLY in the marker issue comment; the PR review body carries
    // non-summary content so a fresh or updated post never shows two summaries.
    let summary_body =
        render_pr_review_markdown(&analyze_result.review, &analyze_result.pr_details.head_sha);
    let review_body =
        render_pr_review_review_body(&analyze_result.review, &analyze_result.pr_details.files);

    if dry_run {
        debug!("Dry-run mode: skipping post");
        eprintln!(
            "Dry-run: Would post {} review to PR #{}",
            event, analyze_result.pr_details.number
        );
        eprintln!("Review body:\n{review_body}");
        eprintln!("Summary comment:\n{summary_body}");
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
            &summary_body,
            &review_body,
            event,
            &analyze_result.review.comments,
            &analyze_result.pr_details.head_sha,
            &analyze_result.pr_details.review_comments,
            !no_dedup_summary,
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
                    "Review summary already up to date for head SHA {}; nothing posted",
                    analyze_result.pr_details.head_sha
                );
            }
            aptu_core::SummaryPostOutcome::Updated => {
                info!(review_id = outcome.review_id, "Review updated successfully");
                eprintln!(
                    "Review updated in place (ID: {}); summary comment updated",
                    outcome.review_id
                );
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

/// Compute reviewability score for a PR.
///
/// Formula: 60% size component + 40% age component.
/// Smaller PRs and older PRs score higher.
///
/// # Arguments
///
/// * `additions` - Number of lines added
/// * `deletions` - Number of lines deleted
/// * `age_days` - Age in days
/// * `max_size` - Maximum size encountered (floor at 500)
///
/// # Returns
///
/// Score in [0.0, 1.0]
#[allow(clippy::cast_precision_loss)]
pub fn compute_score(additions: u64, deletions: u64, age_days: f64, max_size: u64) -> f64 {
    const MIN_MAX_SIZE: u64 = 500;
    let max = max_size.max(MIN_MAX_SIZE).max(1);
    let age = age_days.max(0.0);
    let total_changes = additions.saturating_add(deletions);
    let normalized_size = 1.0 - (std::cmp::min(total_changes, max) as f64 / max as f64);
    let age_norm = (age / 365.0).min(1.0);
    0.6 * normalized_size + 0.4 * age_norm
}

/// Fetch and rank open PRs for a repository.
///
/// Fetches all open PRs, excludes drafts, computes scores, sorts by score DESC
/// (then by number ASC for ties), and applies limit.
///
/// TODO: In follow-up PR, add CI status and conflict detection.
/// TODO: In follow-up PR, add caching of results.
#[instrument(skip_all, fields(repo, limit))]
#[allow(clippy::too_many_lines)]
pub async fn run_queue(
    _config: &aptu_core::AppConfig,
    owner: &str,
    repo: &str,
    limit: u32,
) -> Result<crate::output::pr::PrQueueResult> {
    info!("Fetching open PRs for {}/{}", owner, repo);

    // Create octocrab client
    let client = aptu_core::github::create_client()?;

    // Fetch open PRs (paginated). Cap at MAX_QUEUE_PRS to bound memory and
    // API calls; repos with more open PRs than this cap are uncommon, and
    // the queue command is an interactive advisory tool, not a bulk processor.
    const MAX_QUEUE_PRS: usize = 200;

    let prs_page = client
        .pulls(owner, repo)
        .list()
        .per_page(100)
        .send()
        .await
        .context("Failed to fetch PRs")?;

    let mut all_prs = client
        .all_pages(prs_page)
        .await
        .context("Failed to fetch all PR pages")?;

    if all_prs.len() > MAX_QUEUE_PRS {
        warn!(
            total = all_prs.len(),
            cap = MAX_QUEUE_PRS,
            "Repository has many open PRs; showing top {} by recency",
            MAX_QUEUE_PRS
        );
        all_prs.truncate(MAX_QUEUE_PRS);
    }

    debug!(total_prs = all_prs.len(), "Fetched open PRs");

    // Partition drafts (excluded from queue) from open PRs.
    // SimplePullRequest from the list endpoint does not include additions/deletions;
    // fetch the full PullRequest for each non-draft concurrently to get those fields.
    let mut draft_count = 0usize;
    let mut non_draft_numbers: Vec<u64> = Vec::new();
    for pr in &all_prs {
        if pr.draft.unwrap_or(false) {
            draft_count += 1;
        } else {
            non_draft_numbers.push(pr.number);
        }
    }

    let full_prs = {
        let fetches = non_draft_numbers.iter().map(|&number| {
            let client = client.clone();
            let owner = owner.to_owned();
            let repo = repo.to_owned();
            async move {
                client
                    .pulls(&owner, &repo)
                    .get(number)
                    .await
                    .with_context(|| format!("Failed to fetch PR #{number}"))
            }
        });
        futures::future::try_join_all(fetches).await?
    };

    let now = chrono::Utc::now();

    let mut queued_prs: Vec<crate::output::pr::QueuedPr> = full_prs
        .into_iter()
        .filter_map(|pr| {
            let number = pr.number;
            if number == 0 {
                tracing::warn!("Skipping PR with missing or invalid number; excluding from queue");
                return None;
            }
            #[allow(clippy::cast_precision_loss)]
            let age_days = {
                let created = pr.created_at.unwrap_or_else(chrono::Utc::now);
                let duration = now.signed_duration_since(created);
                duration.num_seconds() as f64 / 86400.0
            };
            let title = pr.title.clone().unwrap_or_default();
            let author = pr
                .user
                .as_ref()
                .map(|u| u.login.clone())
                .unwrap_or_default();
            let additions = pr.additions.unwrap_or(0);
            let deletions = pr.deletions.unwrap_or(0);
            tracing::debug!(
                pr_number = number,
                title = %title,
                author = %author,
                additions,
                deletions,
                "Mapping PR into queue entry"
            );
            Some(crate::output::pr::QueuedPr {
                number,
                title,
                author,
                age_days,
                additions,
                deletions,
                score: 0.0, // Computed below
                draft: false,
            })
        })
        .collect();

    let total_open = queued_prs.len() + draft_count;

    // Compute max_size (floor at 500)
    let max_size = queued_prs
        .iter()
        .map(|pr| pr.additions + pr.deletions)
        .max()
        .unwrap_or(0)
        .max(500);

    // Compute scores and sort
    for pr in &mut queued_prs {
        pr.score = compute_score(pr.additions, pr.deletions, pr.age_days, max_size);
    }

    queued_prs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.number.cmp(&b.number))
    });

    // Apply limit
    if limit > 0 && queued_prs.len() > limit as usize {
        queued_prs.truncate(limit as usize);
    }

    info!(
        prs_in_queue = queued_prs.len(),
        drafts_excluded = draft_count,
        "PR queue computed"
    );

    Ok(crate::output::pr::PrQueueResult {
        prs: queued_prs,
        total_open,
        drafts_excluded: draft_count,
    })
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
    repo_path: Option<std::path::PathBuf>,
    instructions_file: Option<std::path::PathBuf>,
    ctx: crate::cli::OutputContext,
    config: &aptu_core::AppConfig,
    inferred_repo: Option<String>,
    no_dedup_summary: bool,
) -> Result<()> {
    use crate::cli::OutputFormat;

    let repo_path_str = repo_path.map(|p| p.to_string_lossy().into_owned());
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
    let repo_path_str_owned = repo_path_str.clone();
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
            let repo_path_for_review = repo_path_str_owned.clone();
            async move {
                super::review_single_pr(
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
                    repo_path_for_review,
                )
                .await
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

/// Run the PR queue command (spinner and rendering wrapper).
pub async fn run_queue_command(
    repo: Option<String>,
    limit: u32,
    ctx: crate::cli::OutputContext,
    config: &aptu_core::AppConfig,
    inferred_repo: Option<String>,
) -> Result<()> {
    use crate::commands::common::maybe_spinner;

    let repo_context = repo
        .as_deref()
        .or(inferred_repo.as_deref())
        .or(config.user.default_repo.as_deref());

    let repo_str = repo_context.ok_or_else(|| {
        anyhow::anyhow!("Could not determine owner/repo; use --repo or set default_repo in config")
    })?;
    let (owner, repo_name) = aptu_core::github::parse_owner_repo(repo_str)?;

    let spinner = maybe_spinner(&ctx, "Fetching open PRs...");
    let result = run_queue(config, &owner, &repo_name, limit).await?;
    if let Some(s) = spinner {
        s.finish_and_clear();
    }
    crate::output::render(&result, &ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptu_core::ai::types::{CommentSeverity, PrReviewComment};

    #[test]
    fn test_compute_score_small_old_pr() {
        // Small (100 changes), old (365 days)
        let score = compute_score(50, 50, 365.0, 500);
        assert!(score > 0.8, "Old small PR should score high");
    }

    #[test]
    fn test_compute_score_zero_lines() {
        // Edge case: PR with no changes
        // Score = 0.6 * (1.0 - 0/500) + 0.4 * (100/365).min(1.0)
        // = 0.6 * 1.0 + 0.4 * 0.2740 = 0.7096
        let score = compute_score(0, 0, 100.0, 500);
        assert!(
            (score - 0.7096).abs() < 0.001,
            "Zero changes: score = {score}"
        );
    }

    #[test]
    fn test_compute_score_brand_new_pr() {
        // Large PR created today
        let score = compute_score(250, 250, 0.1, 500);
        let normalized_size = 1.0 - (500.0 / 500.0); // size score = 0
        let age_norm = (0.1_f64 / 365.0).min(1.0); // ~0.00027
        let expected = 0.6 * normalized_size + 0.4 * age_norm;
        assert!(
            (score - expected).abs() < 0.001,
            "Score mismatch for brand new PR"
        );
    }

    #[test]
    fn test_compute_score_age_caps_at_one_year() {
        // PR created 2+ years ago
        let score_old = compute_score(100, 100, 730.0, 500);
        let score_one_year = compute_score(100, 100, 365.0, 500);
        assert!(
            (score_old - score_one_year).abs() < 0.001,
            "Age cap at 1.0 should be respected"
        );
    }

    #[test]
    fn test_sort_order_ties_by_number() {
        let mut prs = [
            crate::output::pr::QueuedPr {
                number: 5,
                title: "PR 5".to_string(),
                author: "user".to_string(),
                age_days: 100.0,
                additions: 100,
                deletions: 100,
                score: 0.5,
                draft: false,
            },
            crate::output::pr::QueuedPr {
                number: 3,
                title: "PR 3".to_string(),
                author: "user".to_string(),
                age_days: 100.0,
                additions: 100,
                deletions: 100,
                score: 0.5,
                draft: false,
            },
        ]
        .to_vec();

        prs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.number.cmp(&b.number))
        });

        assert_eq!(
            prs[0].number, 3,
            "Lower PR number should come first when scores are tied"
        );
        assert_eq!(prs[1].number, 5);
    }

    #[test]
    fn test_sort_order_ties_computed_scores() {
        // Use compute_score with identical inputs to produce equal scores,
        // then verify the secondary sort (number ASC) is applied correctly.
        let score = compute_score(100, 50, 90.0, 500);
        let mut prs = [
            crate::output::pr::QueuedPr {
                number: 42,
                title: "PR 42".to_string(),
                author: "alice".to_string(),
                age_days: 90.0,
                additions: 100,
                deletions: 50,
                score,
                draft: false,
            },
            crate::output::pr::QueuedPr {
                number: 7,
                title: "PR 7".to_string(),
                author: "bob".to_string(),
                age_days: 90.0,
                additions: 100,
                deletions: 50,
                score,
                draft: false,
            },
        ]
        .to_vec();
        prs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.number.cmp(&b.number))
        });
        assert_eq!(
            prs[0].number, 7,
            "lower PR number first on tied computed scores"
        );
        assert_eq!(prs[1].number, 42);
    }

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

    #[test]
    fn test_queue_skips_pr_with_zero_number() {
        // Arrange: simulate filter_map logic for a PR with number == 0
        let numbers: Vec<Option<u64>> = vec![Some(0), Some(1), Some(2)];

        // Act: apply the same filter_map guard used in run_queue
        let kept: Vec<u64> = numbers
            .into_iter()
            .filter_map(|n| {
                let number = n.unwrap_or(0);
                if number == 0 {
                    return None;
                }
                Some(number)
            })
            .collect();

        // Assert: PR with number 0 is excluded; valid PRs pass through
        assert_eq!(kept, vec![1, 2]);
    }
}
