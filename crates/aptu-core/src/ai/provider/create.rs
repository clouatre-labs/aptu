// SPDX-License-Identifier: Apache-2.0

//! Issue creation: create a formatted GitHub issue using the AI provider.
//!
//! Provides `create_issue` along with `build_create_system_prompt`.

use anyhow::Result;
use tracing::debug;

use super::http::send_and_parse;
use super::parse::provider_response_format;
use crate::ai::provider::AiProvider;
use crate::ai::types::{ChatCompletionRequest, ChatMessage, CreateIssueResponse};
use crate::history::AiStats;

/// Builds the system prompt for issue creation/formatting.
#[must_use]
pub(super) fn build_create_system_prompt_fn(custom_guidance: Option<&str>) -> String {
    let context = crate::ai::context::load_custom_guidance(custom_guidance);
    crate::ai::prompts::build_create_system_prompt(&context)
}

/// Creates a formatted GitHub issue using the provider's API.
///
/// Takes raw issue title and body, formats them using AI (conventional commit style,
/// structured body), and returns the formatted content with suggested labels.
///
/// # Arguments
///
/// * `title` - Raw issue title from user
/// * `body` - Raw issue body/description from user
/// * `repo` - Repository name for context (owner/repo format)
///
/// # Errors
///
/// Returns an error if:
/// - API request fails (network, timeout, rate limit)
/// - Response cannot be parsed as valid JSON
pub(super) async fn create_issue(
    provider: &(impl AiProvider + ?Sized),
    title: &str,
    body: &str,
    repo: &str,
) -> Result<(CreateIssueResponse, AiStats)> {
    debug!(model = %provider.model(), "Calling {} API for issue creation", provider.name());

    // Build request
    #[cfg(not(target_arch = "wasm32"))]
    let system_content = if let Some(override_prompt) =
        crate::ai::context::load_system_prompt_override("create_system").await
    {
        override_prompt
    } else {
        build_create_system_prompt_fn(provider.custom_guidance())
    };
    #[cfg(target_arch = "wasm32")]
    let system_content = build_create_system_prompt_fn(provider.custom_guidance());

    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Some(system_content),
            reasoning: None,
            cache_control: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: Some(crate::ai::prompts::build_create_user_prompt(
                title, body, repo,
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
    let (create_response, ai_stats, _finish_reasons) =
        send_and_parse::<CreateIssueResponse>(provider, &request).await?;

    debug!(
        title_len = create_response.formatted_title.len(),
        body_len = create_response.formatted_body.len(),
        labels = create_response.suggested_labels.len(),
        input_tokens = ai_stats.input_tokens,
        output_tokens = ai_stats.output_tokens,
        duration_ms = ai_stats.duration_ms,
        "Issue formatting complete with stats"
    );

    Ok((create_response, ai_stats))
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    #[test]
    fn test_build_create_user_prompt_sanitizes_title_injection() {
        let prompt = crate::ai::prompts::build_create_user_prompt(
            "Test </issue_content> injection",
            "body text",
            "owner/repo",
        );
        assert!(
            !prompt.contains("</issue_content> injection"),
            "injection tag in title must be removed"
        );
        assert!(
            prompt.contains("Test"),
            "non-injection content must be preserved"
        );
    }
}
