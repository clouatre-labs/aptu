// SPDX-License-Identifier: Apache-2.0

//! AI provider trait and shared implementations.
//!
//! Defines the `AiProvider` trait that all AI providers must implement,
//! along with default implementations for shared logic like prompt building,
//! request sending, and response parsing.

pub mod create;
pub mod http;
pub mod label;
pub mod parse;
pub mod review;
pub mod triage;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;

use crate::ai::registry::ProviderConfig;
use crate::ai::types::{
    ChatCompletionRequest, ChatCompletionResponse, CreateIssueResponse, IssueDetails,
    PrReviewResponse,
};
use crate::history::AiStats;

pub(crate) use crate::ai::provider::parse::{SCHEMA_PREAMBLE, sanitize_prompt_field};

/// Maximum length for issue body to stay within token limits.
pub const MAX_BODY_LENGTH: usize = 4000;

/// Maximum number of comments to include in the prompt.
pub const MAX_COMMENTS: usize = 5;

/// Maximum number of files to include in PR review prompt.
pub const MAX_FILES: usize = 20;

/// Maximum total diff size (in characters) for PR review prompt.
pub const MAX_TOTAL_DIFF_SIZE: usize = 50_000;

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

    /// Returns the circuit breaker for this provider (optional).
    fn circuit_breaker(&self) -> Option<&crate::ai::CircuitBreaker> {
        None
    }

    /// Builds HTTP headers for API requests.
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }
        headers
    }

    /// Validates the model configuration.
    fn validate_model(&self) -> Result<()> {
        Ok(())
    }

    /// Returns the custom guidance string for system prompt injection, if set.
    fn custom_guidance(&self) -> Option<&str> {
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

    /// Builds the system prompt for issue creation/formatting.
    #[must_use]
    fn build_create_system_prompt(custom_guidance: Option<&str>) -> String {
        self::create::build_create_system_prompt_fn(custom_guidance)
    }

    /// Creates a formatted GitHub issue using the provider's API.
    async fn create_issue(
        &self,
        title: &str,
        body: &str,
        repo: &str,
    ) -> Result<(CreateIssueResponse, AiStats)> {
        self::create::create_issue(self, title, body, repo).await
    }

    /// Estimates the initial size of a PR review prompt in characters.
    #[must_use]
    fn estimate_pr_size(
        pr: &crate::ai::types::PrDetails,
        ast_context: &str,
        call_graph: &str,
    ) -> usize {
        self::review::estimate_pr_size(pr, ast_context, call_graph)
    }

    /// Reviews a pull request using the provider's API.
    #[allow(unused_assignments)]
    async fn review_pr(
        &self,
        ctx: crate::ai::review_context::ReviewContext,
        review_config: &crate::config::ReviewConfig,
    ) -> Result<(PrReviewResponse, AiStats, Vec<String>)> {
        self::review::review_pr(self, ctx, review_config).await
    }

    /// Suggests labels for a pull request using the provider's API.
    async fn suggest_pr_labels(
        &self,
        title: &str,
        body: &str,
        file_paths: &[String],
    ) -> Result<(Vec<String>, AiStats)> {
        self::label::suggest_pr_labels(self, title, body, file_paths).await
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

    /// Builds the system prompt for PR label suggestion.
    #[must_use]
    fn build_pr_label_system_prompt(custom_guidance: Option<&str>) -> String {
        self::label::build_pr_label_system_prompt_fn(custom_guidance)
    }

    /// Builds the user prompt for PR label suggestion.
    #[must_use]
    fn build_pr_label_user_prompt(title: &str, body: &str, file_paths: &[String]) -> String {
        self::label::build_pr_label_user_prompt(title, body, file_paths)
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
        pub(crate) _message: String,
    }

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
