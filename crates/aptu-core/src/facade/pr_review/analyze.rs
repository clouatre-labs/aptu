// SPDX-License-Identifier: Apache-2.0

//! PR review analysis: fetch, diff reconstruction, and AI analysis.

use tracing::{debug, error, instrument};

use crate::ai::provider::AiProvider;
use crate::ai::types::PrDetails;
use crate::auth::TokenProvider;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::load_config;
use crate::config::{AiConfig, TaskType};
use crate::error::AptuError;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::auth::create_client_from_provider;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::pulls::fetch_pr_details;
use crate::sanitize::{redact_secrets, sanitise_user_field};
use crate::security::SecurityScanner;
/// Fetches PR details for review without AI analysis.
///
/// This function handles credential resolution and GitHub API calls,
/// allowing platforms to display PR metadata before starting AI analysis.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or number)
/// * `repo_context` - Optional repository context for bare numbers
///
/// # Returns
///
/// PR details including title, body, files, and labels.
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be fetched
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider), fields(reference = %reference))]
pub async fn fetch_pr_for_review(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
) -> crate::Result<PrDetails> {
    use crate::github::pulls::parse_pr_reference;

    // Parse PR reference
    let (owner, repo, number) =
        parse_pr_reference(reference, repo_context).map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Create GitHub client from provider
    let client = create_client_from_provider(provider)?;

    // Load config to get review settings
    let app_config = load_config().unwrap_or_default();

    // Fetch PR details
    let mut pr = fetch_pr_details(&client, &owner, &repo, number, &app_config.review)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

    // Fetch repository instructions for PR review context
    pr.instructions = crate::github::instructions::fetch_repo_instructions(
        &client,
        &owner,
        &repo,
        &pr.head_sha,
        app_config.review.instructions_file.as_deref(),
        app_config.review.max_instructions_chars,
    )
    .await;

    Ok(pr)
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_pr_for_review(
    _provider: &dyn crate::auth::TokenProvider,
    _reference: &str,
    _repo_context: Option<&str>,
) -> crate::Result<crate::ai::types::PrDetails> {
    crate::facade::wasm_unsupported!("fetch_pr_for_review");
}

/// Reconstructs a unified diff string from PR file patches for security scanning.
///
/// Files with `patch: None` (e.g. binary files or files with no changes) are silently
/// skipped. Patch content is used as-is from the GitHub API response; it is already in
/// unified diff hunk format (`+`/`-`/context lines). Malformed or unexpected patch content
/// degrades gracefully: `scan_diff` only inspects `+`-prefixed lines and ignores anything
/// else, so corrupt hunks are skipped rather than causing errors.
///
/// Total output is capped at `200_000` bytes to bound
/// memory use on PRs with extremely large patches.
fn reconstruct_diff_from_pr(files: &[crate::ai::types::PrFile]) -> String {
    const MAX_RECONSTRUCT_DIFF_SIZE: usize = 200_000;
    let mut diff = String::new();
    for file in files {
        if let Some(patch) = &file.patch {
            // Cap check is intentionally pre-append (soft lower bound, not hard upper bound):
            // it avoids splitting a file header from its patch, which would produce a
            // malformed diff that confuses the scanner's file-path tracking.
            if diff.len() >= MAX_RECONSTRUCT_DIFF_SIZE {
                break;
            }
            diff.push_str("+++ b/");
            diff.push_str(&file.filename);
            diff.push('\n');
            diff.push_str(patch);
            diff.push('\n');
        }
    }
    diff
}

/// Analyzes PR details with AI to generate a review.
///
/// This function takes pre-fetched PR details and performs AI analysis.
/// It should be called after `fetch_pr_for_review()` to allow intermediate display.
///
/// # Arguments
///
/// * `provider` - Token provider for AI credentials
/// * `pr_details` - PR details from `fetch_pr_for_review()`
/// * `ai_config` - AI configuration
///
/// # Returns
///
/// Tuple of (review response, AI stats).
///
/// # Errors
///
/// Returns an error if:
/// - AI provider token is not available from the provider
/// - AI API call fails
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider, pr_details), fields(number = pr_details.number))]
#[allow(clippy::too_many_lines)]
pub async fn analyze_pr(
    provider: &dyn TokenProvider,
    pr_details: &PrDetails,
    ai_config: &AiConfig,
    repo_path: Option<String>,
) -> crate::Result<(
    crate::ai::types::PrReviewResponse,
    crate::history::AiStats,
    crate::metrics::ReviewContextRecord,
)> {
    // Load config once at function entry to ensure consistent review settings
    let app_config = load_config().unwrap_or_default();
    let review_config = app_config.review;

    // Byte-limit pre-check (prompt injection defence)
    // Concatenate all patches, redact sensitive credentials, then validate via sanitise_user_field
    let all_patches: String = pr_details
        .files
        .iter()
        .map(|f| f.patch.as_deref().unwrap_or(""))
        .collect();
    let (redacted_patches, redaction_count) = redact_secrets(&all_patches);
    if redaction_count > 0 {
        debug!(
            redactions = redaction_count,
            "Redacted secrets from PR diff"
        );
    }
    let limit_kb = app_config.prompt.max_diff_bytes / 1024;
    let _ = sanitise_user_field("pr_diff", &redacted_patches, app_config.prompt.max_diff_bytes)
        .map_err(|e| match e {
            AptuError::InputExceedsLimit { field, actual_bytes, limit_bytes, .. } => {
                AptuError::InputExceedsLimit {
                    field,
                    actual_bytes,
                    limit_bytes,
                    hint: format!(" raise `prompt.max_diff_bytes` in ~/.config/aptu/config.toml (current limit: {limit_kb} KiB)"),
                }
            }
            other => other,
        })?;

    // Build review context with all enrichment decisions centralized
    let ctx = crate::ai::review_context::build_review_context(
        pr_details.clone(),
        repo_path,
        &review_config,
    )
    .await?;

    // Emit --verbose pre-flight summary before AI call
    if let Ok(verbose) = std::env::var("APTU_VERBOSE")
        && (verbose == "1" || verbose.to_lowercase() == "true")
    {
        let summary = ctx.verbose_summary();
        if !summary.is_empty() {
            eprintln!("{summary}");
        }
    }

    // Resolve task-specific provider and model
    let (provider_name, model_name) =
        ai_config.resolve_for_task(TaskType::Review, Some(ctx.estimated_size));

    // Pre-AI prompt injection scan (advisory gate)
    let diff = reconstruct_diff_from_pr(&pr_details.files);
    let injection_findings: Vec<_> = SecurityScanner::new()
        .scan_diff(&diff)
        .into_iter()
        .filter(|f| f.pattern_id.starts_with("prompt-injection"))
        .collect();
    if !injection_findings.is_empty() {
        let pattern_ids: Vec<&str> = injection_findings
            .iter()
            .map(|f| f.pattern_id.as_str())
            .collect();
        let message = format!(
            "Prompt injection patterns detected: {}",
            pattern_ids.join(", ")
        );
        error!(patterns = ?pattern_ids, message = %message, "Prompt injection detected; operation blocked");
        return Err(AptuError::SecurityScan { message });
    }

    // Generate trace ID for this review operation
    let trace_id = uuid::Uuid::new_v4().simple().to_string();

    // Use fallback chain if configured
    let (response, mut ai_stats, finish_reasons) = crate::facade::ai_client::try_with_fallback(
        provider,
        &provider_name,
        &model_name,
        ai_config,
        |client| {
            let review_ctx = ctx.clone();
            let review_cfg = review_config.clone();
            async move { client.review_pr(review_ctx, &review_cfg).await }
        },
    )
    .await?;

    // Set trace_id on ai_stats
    ai_stats.trace_id = Some(trace_id.clone());

    // Build ReviewContextRecord from context and response metadata
    let context_record = crate::metrics::ReviewContextRecord {
        trace_id,
        operation: "pr_review".to_string(),
        pr: format!(
            "{}/{}#{}",
            pr_details.owner, pr_details.repo, pr_details.number
        ),
        model: ai_stats.model.clone(),
        github_actor: std::env::var("GITHUB_ACTOR").ok(),
        files_total: ctx.files_total,
        files_with_patch: ctx.files_with_patch,
        files_truncated: ctx.files_truncated,
        truncated_chars_dropped: ctx.truncated_chars_dropped,
        ast_context_chars: ctx.ast_context.len(),
        call_graph_chars: ctx.call_graph.len(),
        dep_enrichments_count: ctx.dep_enrichments_count,
        dep_enrichments_chars: ctx.dep_enrichments_chars,
        budget_drops: ctx.budget_drops,
        cwd_inferred: ctx.cwd_inferred,
        prompt_chars_final: ai_stats.prompt_chars,
        finish_reasons,
        max_prompt_chars: review_config.max_prompt_chars,
    };

    Ok((response, ai_stats, context_record))
}

#[cfg(target_arch = "wasm32")]
pub async fn analyze_pr(
    _provider: &dyn crate::auth::TokenProvider,
    _pr_details: &crate::ai::types::PrDetails,
    _ai_config: &crate::config::AiConfig,
    _repo_path: Option<String>,
) -> crate::Result<(
    crate::ai::types::PrReviewResponse,
    crate::history::AiStats,
    crate::metrics::ReviewContextRecord,
)> {
    crate::facade::wasm_unsupported!("analyze_pr");
}

/// Decision returned by [`dedup_outcome`] for how to handle an outgoing inline comment
/// against the dedup map of existing bot-authored comments.
#[derive(Debug)]
pub(crate) enum DedupOutcome {
    /// No existing comment with the same key; the comment should be posted.
    Post,
    /// Existing comment body matches; skip the comment.
    Skip,
    /// Existing comment body differs; update it in place with the given body.
    Update {
        comment_id: u64,
        rendered_body: String,
    },
}
