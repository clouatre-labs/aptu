// SPDX-License-Identifier: Apache-2.0

//! PR review command handler.
//!
//! Fetches a pull request, analyzes it with AI, and displays
//! structured review feedback locally. Optionally posts the review to GitHub.
//! Split into `fetch()` and `analyze()` for proper display flow (show PR details
//! before AI spinner).

use anyhow::{Context, Result};
use aptu_core::ai::types::PrReviewComment;
use aptu_core::{
    PrDetails, PrReviewResponse, render_pr_review_comment_body, render_pr_review_markdown,
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
    deep: bool,
) -> Result<(PrReviewResponse, aptu_core::history::AiStats)> {
    // Create CLI token provider
    let provider = CliTokenProvider;

    // Call facade for analysis
    let (review, ai_stats) =
        aptu_core::analyze_pr(&provider, pr_details, ai_config, repo_path, deep).await?;

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
#[allow(clippy::too_many_arguments)]
pub async fn run_pr_create(
    repo: Option<String>,
    inferred_repo: Option<String>,
    default_repo: Option<String>,
    title: String,
    body: Option<String>,
    branch: Option<String>,
    base: String,
    diff: Option<std::path::PathBuf>,
    draft: bool,
    force: bool,
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
    let mut head = if let Some(b) = branch {
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

    // If diff is provided, apply the patch and get the branch name
    if let Some(patch_path) = diff {
        let repo_root = std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get current directory: {e}"))?;

        let progress = |_step: aptu_core::PatchStep| {
            // Suppress progress in text output mode; could show if is_interactive()
        };

        // Load config to determine dco_signoff setting for this repo
        let config = aptu_core::load_config().context("Failed to load configuration")?;
        let dco_signoff = config.repos.dco_signoff;

        head = aptu_core::apply_patch_and_push(
            &patch_path,
            &repo_root,
            None,
            &base,
            &title,
            dco_signoff,
            force,
            progress,
        )
        .await?;
    }

    let provider = CliTokenProvider;

    aptu_core::create_pr(
        &provider,
        &owner,
        &repo_name,
        &title,
        &base,
        &head,
        body.as_deref(),
        draft,
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
    let max = max_size.max(MIN_MAX_SIZE);
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

    // Map to QueuedPr and track drafts
    let mut queued_prs: Vec<crate::output::pr::QueuedPr> = Vec::new();
    let mut draft_count = 0;

    let now = chrono::Utc::now();

    for pr in all_prs {
        let is_draft = pr.draft.unwrap_or(false);
        if is_draft {
            draft_count += 1;
            continue;
        }

        let number = pr.number;
        let title = pr.title.unwrap_or_default();
        let author = pr
            .user
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |u| u.login.clone());

        #[allow(clippy::cast_precision_loss)]
        let age_days = pr.created_at.map_or(0.0, |created| {
            let duration = now.signed_duration_since(created);
            duration.num_seconds() as f64 / 86400.0
        });

        let additions = pr.additions.unwrap_or(0);
        let deletions = pr.deletions.unwrap_or(0);

        queued_prs.push(crate::output::pr::QueuedPr {
            number,
            title,
            author,
            age_days,
            additions,
            deletions,
            score: 0.0, // Computed below
            draft: false,
        });
    }

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
            "Zero changes: score = {}",
            score
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
        let mut prs = vec![
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
        ];

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
