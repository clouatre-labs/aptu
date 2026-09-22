// SPDX-License-Identifier: Apache-2.0

//! PR auto-labeling facade.

use tracing::{debug, instrument};

#[cfg(not(target_arch = "wasm32"))]
use super::post::{fetch_repo_viewer_permission_cached, pr_write_allowed};
use crate::ai::provider::AiProvider;
use crate::auth::TokenProvider;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::load_config;
use crate::config::{AiConfig, TaskType};
use crate::error::AptuError;
use crate::facade::issues::WriteOutcome;
#[cfg(not(target_arch = "wasm32"))]
use crate::github::auth::create_client_from_provider;
use crate::sanitize::{redact_secrets, sanitise_user_field};
/// Outcome payload for [`label_pr`]: PR number, title, URL, applied labels, and AI stats.
pub type LabelPrOutcome = (u64, String, String, Vec<String>, crate::history::AiStats);

/// Auto-label a pull request based on conventional commit prefix and file paths.
///
/// Fetches PR details, extracts labels from title and changed files,
/// and applies them to the PR. Optionally previews without applying.
///
/// # Arguments
///
/// * `provider` - Token provider for GitHub credentials
/// * `reference` - PR reference (URL, owner/repo#number, or bare number)
/// * `repo_context` - Optional repository context for bare numbers
/// * `dry_run` - If true, preview labels without applying
///
/// # Returns
///
/// Tuple of (`pr_number`, `pr_title`, `pr_url`, `labels`).
///
/// # Errors
///
/// Returns an error if:
/// - GitHub token is not available from the provider
/// - PR cannot be parsed or found
/// - API call fails
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(provider), fields(reference = %reference))]
#[allow(clippy::too_many_lines)]
pub async fn label_pr(
    provider: &dyn TokenProvider,
    reference: &str,
    repo_context: Option<&str>,
    dry_run: bool,
    ai_config: &AiConfig,
) -> crate::Result<WriteOutcome<LabelPrOutcome>> {
    use crate::github::issues::apply_labels_to_number;
    use crate::github::pulls::{fetch_pr_details, labels_from_pr_metadata, parse_pr_reference};

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
    let pr_details = fetch_pr_details(&client, &owner, &repo, number, &app_config.review)
        .await
        .map_err(|e| AptuError::GitHub {
            message: e.to_string(),
        })?;

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
    let _ = sanitise_user_field(
        "pr_diff",
        &redacted_patches,
        app_config.prompt.max_diff_bytes,
    )?;

    // Extract labels from PR metadata (deterministic approach)
    let file_paths: Vec<String> = pr_details
        .files
        .iter()
        .map(|f| f.filename.clone())
        .collect();
    let mut labels = labels_from_pr_metadata(&pr_details.title, &file_paths);
    let mut ai_stats: Option<crate::history::AiStats> = None;

    // If no labels found, try AI fallback
    if labels.is_empty() {
        // Resolve task-specific provider and model for Create task
        let (provider_name, model_name) = ai_config.resolve_for_task(TaskType::Create, None);

        // Get API key from provider using the resolved provider name
        if let Some(api_key) = provider.ai_api_key(&provider_name) {
            // Apply the per-task effective timeout for the Create task
            let mut task_config = ai_config.clone();
            task_config.timeout_seconds = ai_config.effective_timeout_for_task(TaskType::Create);

            // Create AI client with resolved provider and model
            if let Ok(ai_client) = crate::ai::AiClient::with_api_key(
                &provider_name,
                api_key,
                &model_name,
                &task_config,
            ) {
                match ai_client
                    .suggest_pr_labels(&pr_details.title, &pr_details.body, &file_paths)
                    .await
                {
                    Ok((ai_labels, stats)) => {
                        labels = ai_labels;
                        ai_stats = Some(stats);
                        debug!("AI fallback provided {} labels", labels.len());
                    }
                    Err(e) => {
                        debug!("AI fallback failed: {}", e);
                        // Continue without labels rather than failing
                    }
                }
            }
        }
    }

    // If no AI stats were captured, create a default one
    let stats = ai_stats.unwrap_or_else(|| {
        crate::history::AiStats {
            provider: "unknown".to_string(),
            model: "unknown".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
            cost_usd: None,
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        }
        .with_computed_etu()
    });

    // Apply labels if not dry-run
    if !dry_run && !labels.is_empty() {
        // Gate on viewer permission: skip with an informational signal when denied
        let perm = fetch_repo_viewer_permission_cached(&client, &owner, &repo).await;
        if !pr_write_allowed(perm) {
            tracing::info!(
                repo = %format!("{owner}/{repo}"),
                "Viewer lacks write access; skipping PR labeling"
            );
            return Ok(WriteOutcome::Skipped);
        }

        apply_labels_to_number(&client, &owner, &repo, number, &labels)
            .await
            .map_err(crate::error::aptu_error_from_anyhow)?;
    }

    Ok(WriteOutcome::Applied((
        number,
        pr_details.title,
        pr_details.url,
        labels,
        stats,
    )))
}

#[cfg(target_arch = "wasm32")]
pub async fn label_pr(
    _provider: &dyn crate::auth::TokenProvider,
    _reference: &str,
    _repo_context: Option<&str>,
    _dry_run: bool,
    _ai_config: &crate::config::AiConfig,
) -> crate::Result<WriteOutcome<LabelPrOutcome>> {
    crate::facade::wasm_unsupported!("label_pr");
}
