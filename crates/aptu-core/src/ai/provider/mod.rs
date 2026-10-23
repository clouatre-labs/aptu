// SPDX-License-Identifier: Apache-2.0

//! AI provider trait and shared implementations.
//!
//! Defines the `AiProvider` trait that all AI providers must implement,
//! along with default implementations for shared logic like prompt building,
//! request sending, and response parsing.

pub mod http;
pub mod parse;
pub mod review;
pub mod triage;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use crate::ai::registry::ProviderConfig;
use crate::ai::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, IssueDetails, PrReviewResponse,
};
use crate::metrics::AiStats;

pub(super) use crate::ai::provider::parse::provider_response_format;
pub(crate) use crate::ai::provider::parse::{SCHEMA_PREAMBLE, sanitize_prompt_field};

/// Builds a [`ChatCompletionRequest`] with the system and user messages.
///
/// Injects cache control on the system message for Anthropic providers.
///
/// # Arguments
///
/// * `provider` - The AI provider (supplies model, limits, and session ID).
/// * `system_content` - The assembled system prompt content.
/// * `user_content` - The assembled user prompt content.
/// * `session_kind` - Session kind passed to `session_id` (e.g. "triage", "review").
#[must_use]
pub(super) fn build_chat_request(
    provider: &(impl AiProvider + ?Sized),
    system_content: String,
    user_content: String,
    session_kind: &str,
) -> ChatCompletionRequest {
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Some(system_content),
            reasoning: None,
            cache_control: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: Some(user_content),
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

    ChatCompletionRequest {
        model: provider.model().to_string(),
        messages,
        response_format: provider_response_format(provider),
        max_tokens: Some(provider.max_tokens()),
        temperature: Some(provider.temperature()),
        session_id: provider.session_id(session_kind),
    }
}

/// Maximum length for issue body to stay within token limits.
pub const MAX_BODY_LENGTH: usize = 4000;

/// Maximum number of comments to include in the prompt.
pub const MAX_COMMENTS: usize = 5;

/// Maximum number of files to include in PR review prompt.
pub const MAX_FILES: usize = 20;

/// Maximum number of labels to include in the prompt.
pub const MAX_LABELS: usize = 30;

/// Maximum number of milestones to include in the prompt.
pub const MAX_MILESTONES: usize = 10;

/// AI provider trait for issue triage and creation.
///
/// Defines the interface that all AI providers must implement.
/// Default implementations are provided for shared logic.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait AiProvider: Send + Sync {
    /// Returns the provider configuration.
    fn config(&self) -> &ProviderConfig;

    /// Returns the name of the provider (e.g., "gemini", "openrouter").
    fn name(&self) -> &str {
        self.config().name
    }

    /// Returns the API URL for this provider.
    fn api_url(&self) -> &str {
        self.config().api_url
    }

    /// Returns the environment variable name for the API key.
    fn api_key_env(&self) -> &str {
        self.config().api_key_env
    }

    /// Returns the HTTP client for making requests.
    fn http_client(&self) -> &Client;

    /// Returns the API key for authentication.
    fn api_key(&self) -> &SecretString;

    /// Returns the model name.
    fn model(&self) -> &str {
        self.config().model
    }

    /// Returns the maximum tokens for API responses.
    fn max_tokens(&self) -> u32 {
        self.config().max_tokens
    }

    /// Returns the temperature for API requests.
    fn temperature(&self) -> f32 {
        self.config().temperature
    }

    /// Returns whether this provider is Anthropic-compatible and supports
    /// `cache_control` on message blocks.
    fn is_anthropic(&self) -> bool {
        self.name() == crate::ai::registry::PROVIDER_ANTHROPIC
    }

    /// Returns the maximum retry attempts for rate-limited requests.
    fn max_attempts(&self) -> u32 {
        3
    }

    /// Builds HTTP headers for API requests.
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }
        headers
    }

    /// Provider-specific request body extensions (e.g., `OpenRouter` data controls).
    fn provider_body_extensions(&self) -> Option<serde_json::Value> {
        None
    }

    /// Validates the model configuration.
    fn validate_model(&self) -> Result<()> {
        Ok(())
    }

    /// Returns the custom guidance string for system prompt injection, if set.
    fn custom_guidance(&self) -> Option<&str> {
        None
    }

    /// Returns a stable session identifier for sticky provider routing, if supported.
    fn session_id(&self, _task_type: &str) -> Option<String> {
        None
    }

    /// Sends a chat completion request to the provider's API (HTTP-only, no retry).
    #[allow(private_interfaces)]
    async fn send_request_inner(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        self::http::send_request_inner(self, request).await
    }

    /// Sends a chat completion request and parses the response with retry logic.
    #[allow(private_interfaces)]
    async fn send_and_parse<T: serde::de::DeserializeOwned + Send>(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<(T, AiStats, Vec<String>)> {
        self::http::send_and_parse(self, request).await
    }

    /// Analyzes a GitHub issue using the provider's API.
    async fn analyze_issue(&self, issue: &IssueDetails) -> Result<crate::ai::AiResponse> {
        self::triage::analyze_issue(self, issue).await
    }

    /// Builds the system prompt for issue triage.
    #[must_use]
    fn build_system_prompt(custom_guidance: Option<&str>) -> String {
        self::triage::build_system_prompt(custom_guidance)
    }

    /// Reviews a pull request using the provider's API.
    #[allow(unused_assignments)]
    async fn review_pr(
        &self,
        ctx: crate::ai::review_context::ReviewContext,
        review_config: &crate::config::ReviewConfig,
    ) -> Result<(PrReviewResponse, AiStats, Vec<String>, Vec<String>)> {
        self::review::review_pr(self, ctx, review_config).await
    }

    /// Verifies candidate review findings against the diff with one AI call.
    async fn verify_findings(
        &self,
        comments: &[crate::ai::types::PrReviewComment],
        diff: &str,
        review_config: &crate::config::ReviewConfig,
    ) -> Result<(
        crate::ai::types::VerifyFindingsResponse,
        AiStats,
        Vec<String>,
    )> {
        self::review::verify_findings(self, comments, diff, review_config).await
    }

    /// Builds the system prompt for PR review.
    #[must_use]
    fn build_pr_review_system_prompt(custom_guidance: Option<&str>) -> String {
        self::review::build_pr_review_system_prompt_fn(custom_guidance)
    }

    /// Builds the user prompt for PR review.
    #[must_use]
    fn build_pr_review_user_prompt(ctx: &mut crate::ai::review_context::ReviewContext) -> String {
        self::review::build_pr_review_user_prompt(ctx)
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;

    pub(crate) static TEST_PROVIDER_CONFIG: ProviderConfig = ProviderConfig {
        name: "test",
        display_name: "Test",
        api_url: "https://test.example.com",
        api_key_env: "TEST_API_KEY",
        model: "test-model",
        max_tokens: 2048,
        temperature: 0.3,
    };

    #[derive(Debug, serde::Deserialize)]
    pub(crate) struct ErrorTestResponse {
        #[serde(rename = "_message")]
        pub(crate) message: String,
    }

    #[allow(dead_code)]
    pub(crate) struct TestProvider;

    impl AiProvider for TestProvider {
        fn config(&self) -> &ProviderConfig {
            &TEST_PROVIDER_CONFIG
        }

        fn http_client(&self) -> &Client {
            unimplemented!()
        }

        fn api_key(&self) -> &SecretString {
            unimplemented!()
        }
    }
}
