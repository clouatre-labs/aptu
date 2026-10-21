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

/// Resolves the minimum context window (in tokens) across the selected model
/// and any configured fallback models, so the prompt budget respects the
/// smallest window the request could target.
///
/// Semantics are intentionally conservative: a model whose registry lookup
/// fails (provider registry unreachable, or model absent from the listing) is
/// EXCLUDED from the minimum rather than silently continuing with the known
/// windows. Only when no model at all resolves (selected plus every fallback)
/// does this degrade to `None` (treated as unbounded), with a debug log. A
/// model that resolves but advertises no `context_window` is also treated as
/// unbounded and does not constrain the minimum.
/// Folds the context windows of `models` resolved from the registry listing
/// `listed` into `min_window` (conservatively excluding models that are absent
/// from the listing). Returns the updated minimum and the number of models
/// that resolved.
#[cfg(not(target_arch = "wasm32"))]
fn fold_model_windows(
    listed: &[crate::ai::registry::CachedModel],
    prov: &str,
    models: &[&str],
    mut min_window: Option<u32>,
) -> (Option<u32>, usize) {
    let mut resolved = 0;
    for model in models {
        // Conservative: a model that cannot be resolved in the registry is
        // excluded from the minimum instead of silently continuing with the
        // known windows.
        match listed.iter().find(|m| m.id == *model) {
            Some(found) => {
                resolved += 1;
                if let Some(window) = found.context_window {
                    min_window = Some(min_window.map_or(window, |cur| cur.min(window)));
                }
            }
            None => {
                debug!(
                    provider = prov,
                    model = model,
                    "Model not found in registry; excluded from context-window cap"
                );
            }
        }
    }
    (min_window, resolved)
}

#[cfg(not(target_arch = "wasm32"))]
async fn resolve_context_window(
    provider: &dyn TokenProvider,
    ai_config: &AiConfig,
    provider_name: &str,
    model_name: &str,
) -> Option<u32> {
    use crate::ai::registry::{CachedModelRegistry, ModelRegistry};
    use std::collections::BTreeMap;

    let registry = CachedModelRegistry::new(
        crate::cache::cache_dir(),
        crate::cache::DEFAULT_MODEL_TTL_SECS,
        provider,
    );

    // Collect (provider -> models) pairs: selected model plus the fallback chain.
    let mut targets: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    targets.entry(provider_name).or_default().push(model_name);
    if let Some(fallback) = &ai_config.fallback {
        for entry in &fallback.chain {
            targets
                .entry(entry.provider.as_str())
                .or_default()
                .push(entry.model.as_deref().unwrap_or(ai_config.model.as_str()));
        }
    }

    let mut min_window: Option<u32> = None;
    let mut resolved_any = false;
    for (prov, models) in targets {
        match registry.list_models(prov).await {
            Ok(listed) => {
                let (folded, resolved) = fold_model_windows(&listed, prov, &models, min_window);
                min_window = folded;
                resolved_any = resolved_any || resolved > 0;
            }
            Err(e) => {
                debug!(
                    provider = prov,
                    error = %e,
                    "Model registry unavailable; provider models excluded from context-window cap"
                );
            }
        }
    }
    if !resolved_any {
        debug!(
            "No models resolved from registry (selected plus fallbacks); context window treated as unbounded"
        );
        return None;
    }
    min_window
}

/// Computes a pre-context routing size estimate: the user-prompt content plus
/// the static default system-prompt overhead. The exact system prompt sent by
/// `review_pr` depends on the resolved provider's custom guidance (and the
/// cfg-gated override file), so routing cannot use it without creating a
/// circular dependency (estimate -> resolve -> build). Routing instead uses
/// this static approximation, which is accurate for the default prompt.
#[cfg(not(target_arch = "wasm32"))]
fn routing_size_estimate(pr_details: &PrDetails, instructions_overhead: usize) -> usize {
    let default_system_prompt = crate::ai::prompts::build_pr_review_system_prompt(
        &crate::ai::context::load_custom_guidance(None),
    );
    crate::ai::review_context::estimate_pr_size(
        pr_details,
        "",
        "",
        default_system_prompt.len() + instructions_overhead,
    )
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

    // Resolve task-specific provider and model BEFORE building the review
    // context, so the selected model's registry context window can cap the
    // prompt budget. Routing uses a pre-context size estimate (user-prompt
    // content plus the static default system-prompt overhead) so
    // estimated-size-based model-tier routing keeps working; the exact system
    // prompt depends on the resolved provider, so the static approximation
    // breaks that circular dependency (see `routing_size_estimate`).
    let instructions_overhead = pr_details
        .instructions
        .as_deref()
        .map_or(0, |instructions| {
            let escaped = instructions
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            format!("<repo_instructions>\n{escaped}\n</repo_instructions>\n\n").len()
        });
    let routing_estimate = routing_size_estimate(pr_details, instructions_overhead);
    let (provider_name, model_name) =
        ai_config.resolve_for_task(TaskType::Review, Some(routing_estimate));
    let context_window =
        resolve_context_window(provider, ai_config, &provider_name, &model_name).await;

    // Build review context with all enrichment decisions centralized
    let ctx = crate::ai::review_context::build_review_context(
        pr_details.clone(),
        repo_path,
        &review_config,
        context_window,
        ai_config.max_tokens,
        ai_config.custom_guidance.as_deref(),
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

    // Use fallback chain if configured (provider/model resolved pre-context above)
    let (response, mut ai_stats, finish_reasons, truncated_patch_files) =
        crate::facade::ai_client::try_with_fallback(
            provider,
            &provider_name,
            &model_name,
            ai_config,
            TaskType::Review,
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
        truncated_patch_files,
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// Minimal `PrDetails` for routing-estimate tests.
    fn make_pr() -> PrDetails {
        PrDetails {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "title".to_string(),
            body: "body".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: String::new(),
            files: vec![],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        }
    }

    /// Verifies the pre-context routing estimate is non-zero (so
    /// `resolve_for_task` receives a non-`None`, meaningful size) and that the
    /// instructions overhead is included additively.
    #[test]
    fn test_routing_size_estimate_positive_and_includes_overhead() {
        let pr = make_pr();
        let base = routing_size_estimate(&pr, 0);
        assert!(
            base > 0,
            "routing estimate must be non-zero so model-tier routing engages"
        );
        let with_overhead = routing_size_estimate(&pr, 5_000);
        assert_eq!(
            with_overhead - base,
            5_000,
            "instructions overhead must be included additively in the routing estimate"
        );
    }

    /// Verifies the conservative fallback fold: models absent from the registry
    /// listing are excluded, the minimum is taken over resolved windows, and
    /// when nothing resolves there is no minimum (degrades to unbounded).
    #[test]
    fn test_fold_model_windows_excludes_unresolved_and_takes_min() {
        use crate::ai::registry::CachedModel;
        let make_model = |id: &str, window: u32| CachedModel {
            id: id.to_string(),
            name: None,
            is_free: None,
            context_window: Some(window),
            provider: "openrouter".to_string(),
            capabilities: vec![],
            pricing: None,
        };
        let listed = [make_model("small", 200_000), make_model("tiny", 8_000)];

        // "missing" is not in the listing, so it must be excluded; the min is
        // taken over the resolved windows only.
        let (min_window, resolved) =
            fold_model_windows(&listed, "openrouter", &["small", "missing", "tiny"], None);
        assert_eq!(resolved, 2, "unresolved models must not count as resolved");
        assert_eq!(min_window, Some(8_000), "min must be over resolved windows");

        // No model resolves -> no minimum (degrades to unbounded upstream).
        let (none_min, none_resolved) =
            fold_model_windows(&listed, "openrouter", &["missing"], None);
        assert_eq!(none_resolved, 0);
        assert_eq!(none_min, None);
    }
}
