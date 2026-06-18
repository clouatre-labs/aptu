// SPDX-License-Identifier: Apache-2.0

//! PR label suggestion: suggest labels for a pull request using the AI provider.
//!
//! Provides `suggest_pr_labels` and prompt builder helpers.

use anyhow::Result;
use tracing::{debug, instrument};

use super::http::send_and_parse;
use super::parse::provider_response_format;
use crate::ai::provider::AiProvider;
use crate::ai::types::{ChatCompletionRequest, ChatMessage, PrLabelResponse};
use crate::history::AiStats;

/// Builds the system prompt for PR label suggestion.
#[must_use]
pub(super) fn build_pr_label_system_prompt_fn(custom_guidance: Option<&str>) -> String {
    let context = crate::ai::context::load_custom_guidance(custom_guidance);
    crate::ai::prompts::build_pr_label_system_prompt(&context)
}

/// Builds the user prompt for PR label suggestion.
#[must_use]
pub(super) fn build_pr_label_user_prompt(title: &str, body: &str, file_paths: &[String]) -> String {
    crate::ai::prompts::build_pr_label_user_prompt(title, body, file_paths)
}

/// Suggests labels for a pull request using the provider's API.
///
/// Analyzes PR title, body, and file paths to suggest relevant labels.
///
/// # Arguments
///
/// * `title` - Pull request title
/// * `body` - Pull request description
/// * `file_paths` - List of file paths changed in the PR
///
/// # Errors
///
/// Returns an error if:
/// - API request fails (network, timeout, rate limit)
/// - Response cannot be parsed as valid JSON
#[instrument(skip(provider), fields(title = %title))]
pub(super) async fn suggest_pr_labels(
    provider: &(impl AiProvider + ?Sized),
    title: &str,
    body: &str,
    file_paths: &[String],
) -> Result<(Vec<String>, AiStats)> {
    debug!(model = %provider.model(), "Calling {} API for PR label suggestion", provider.name());

    // Build request
    #[cfg(not(target_arch = "wasm32"))]
    let system_content = if let Some(override_prompt) =
        crate::ai::context::load_system_prompt_override("pr_label_system").await
    {
        override_prompt
    } else {
        build_pr_label_system_prompt_fn(provider.custom_guidance())
    };
    #[cfg(target_arch = "wasm32")]
    let system_content = build_pr_label_system_prompt_fn(provider.custom_guidance());

    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Some(system_content),
            reasoning: None,
            cache_control: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: Some(crate::ai::prompts::build_pr_label_user_prompt(
                title, body, file_paths,
            )),
            reasoning: None,
            cache_control: None,
        },
    ];

    // Inject cache control on system message for Anthropic
    if provider.is_anthropic()
        && let Some(msg) = messages.first_mut()
    {
        msg.cache_control = Some(crate::ai::types::CacheControl::ephemeral());
    }

    let request = ChatCompletionRequest {
        model: provider.model().to_string(),
        messages,
        response_format: provider_response_format(provider),
        max_tokens: Some(provider.max_tokens()),
        temperature: Some(provider.temperature()),
    };

    // Send request and parse JSON with retry logic
    let (response, ai_stats, _finish_reasons) =
        send_and_parse::<PrLabelResponse>(provider, &request).await?;

    debug!(
        label_count = response.suggested_labels.len(),
        input_tokens = ai_stats.input_tokens,
        output_tokens = ai_stats.output_tokens,
        duration_ms = ai_stats.duration_ms,
        "PR label suggestion complete with stats"
    );

    Ok((response.suggested_labels, ai_stats))
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    #[test]
    fn test_build_pr_label_user_prompt_with_title_and_body() {
        let prompt =
            crate::ai::prompts::build_pr_label_user_prompt("Fix bug", "Bug description", &[]);
        assert!(prompt.contains("Fix bug"));
        assert!(prompt.contains("Bug description"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_empty_body() {
        let prompt = build_pr_label_user_prompt("Fix", "", &[]);
        assert!(prompt.contains("Fix"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_truncates_long_body() {
        let long_body = "x".repeat(5000);
        let prompt = build_pr_label_user_prompt("Fix", &long_body, &[]);
        assert!(
            prompt.len() < long_body.len(),
            "label prompt body should be truncated"
        );
    }

    #[test]
    fn test_build_pr_label_user_prompt_respects_file_limit() {
        let files: Vec<String> = (0..30).map(|i| format!("file{i}.rs")).collect();
        let prompt = build_pr_label_user_prompt("Fix", "body", &files);
        assert!(prompt.contains("file0.rs"), "first file should be included");
        let count = prompt.matches("file").count();
        assert!(
            count <= crate::ai::provider::MAX_LABELS + 5,
            "should not include too many files beyond limit"
        );
    }

    #[test]
    fn test_build_pr_label_user_prompt_empty_files() {
        let prompt = build_pr_label_user_prompt("Fix", "body", &[]);
        assert!(prompt.contains("Fix"), "title should still appear");
        assert!(prompt.contains("body"), "body should still appear");
    }
}
