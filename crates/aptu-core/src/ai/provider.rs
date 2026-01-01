// SPDX-License-Identifier: Apache-2.0

//! AI provider trait and shared implementations.
//!
//! Defines the `AiProvider` trait that all AI providers must implement,
//! along with default implementations for shared logic like prompt building,
//! request sending, and response parsing.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::SecretString;
use tracing::{debug, instrument};

use super::AiResponse;
use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, IssueDetails, ResponseFormat,
    TriageResponse,
};
use crate::history::AiStats;

/// Parses JSON response from AI provider, detecting truncated responses.
///
/// If the JSON parsing fails with an EOF error (indicating the response was cut off),
/// returns a `TruncatedResponse` error that can be retried. Other JSON errors are
/// wrapped as `InvalidAIResponse`.
///
/// # Arguments
///
/// * `text` - The JSON text to parse
/// * `provider` - The name of the AI provider (for error context)
///
/// # Returns
///
/// Parsed value of type T, or an error if parsing fails
fn parse_ai_json<T: serde::de::DeserializeOwned>(text: &str, provider: &str) -> Result<T> {
    match serde_json::from_str::<T>(text) {
        Ok(value) => Ok(value),
        Err(e) => {
            // Check if this is an EOF error (truncated response)
            if e.is_eof() {
                Err(anyhow::anyhow!(
                    crate::error::AptuError::TruncatedResponse {
                        provider: provider.to_string(),
                    }
                ))
            } else {
                Err(anyhow::anyhow!(crate::error::AptuError::InvalidAIResponse(
                    e
                )))
            }
        }
    }
}

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
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Returns the name of the provider (e.g., "gemini", "openrouter").
    fn name(&self) -> &str;

    /// Returns the API URL for this provider.
    fn api_url(&self) -> &str;

    /// Returns the environment variable name for the API key.
    fn api_key_env(&self) -> &str;

    /// Returns the HTTP client for making requests.
    fn http_client(&self) -> &Client;

    /// Returns the API key for authentication.
    fn api_key(&self) -> &SecretString;

    /// Returns the model name.
    fn model(&self) -> &str;

    /// Returns the maximum tokens for API responses.
    fn max_tokens(&self) -> u32;

    /// Returns the temperature for API requests.
    fn temperature(&self) -> f32;

    /// Returns the circuit breaker for this provider (optional).
    ///
    /// Default implementation returns None. Providers can override
    /// to provide circuit breaker functionality.
    fn circuit_breaker(&self) -> Option<&super::CircuitBreaker> {
        None
    }

    /// Builds HTTP headers for API requests.
    ///
    /// Default implementation includes Authorization and Content-Type headers.
    /// Providers can override to add custom headers.
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = "application/json".parse() {
            headers.insert("Content-Type", val);
        }
        headers
    }

    /// Validates the model configuration.
    ///
    /// Default implementation does nothing. Providers can override
    /// to enforce constraints (e.g., free tier validation).
    fn validate_model(&self) -> Result<()> {
        Ok(())
    }

    /// Sends a chat completion request to the provider's API (HTTP-only, no retry).
    ///
    /// Default implementation handles HTTP headers, error responses (401, 429).
    /// Does not include retry logic - use `send_and_parse()` for retry behavior.
    async fn send_request_inner(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        use secrecy::ExposeSecret;
        use tracing::warn;

        use crate::error::AptuError;

        let mut req = self.http_client().post(self.api_url());

        // Add Authorization header
        req = req.header(
            "Authorization",
            format!("Bearer {}", self.api_key().expose_secret()),
        );

        // Add custom headers from provider
        for (key, value) in &self.build_headers() {
            req = req.header(key.clone(), value.clone());
        }

        let response = req
            .json(request)
            .send()
            .await
            .context(format!("Failed to send request to {} API", self.name()))?;

        // Check for HTTP errors
        let status = response.status();
        if !status.is_success() {
            if status.as_u16() == 401 {
                anyhow::bail!(
                    "Invalid {} API key. Check your {} environment variable.",
                    self.name(),
                    self.api_key_env()
                );
            } else if status.as_u16() == 429 {
                warn!("Rate limited by {} API", self.name());
                // Parse Retry-After header (seconds), default to 0 if not present
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                debug!(retry_after, "Parsed Retry-After header");
                return Err(AptuError::RateLimited {
                    provider: self.name().to_string(),
                    retry_after,
                }
                .into());
            }
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "{} API error (HTTP {}): {}",
                self.name(),
                status.as_u16(),
                error_body
            );
        }

        // Parse response
        let completion: ChatCompletionResponse = response
            .json()
            .await
            .context(format!("Failed to parse {} API response", self.name()))?;

        Ok(completion)
    }

    /// Sends a chat completion request and parses the response with retry logic.
    ///
    /// This method wraps both HTTP request and JSON parsing in a single retry loop,
    /// allowing truncated responses to be retried. Includes circuit breaker handling.
    ///
    /// # Arguments
    ///
    /// * `request` - The chat completion request to send
    ///
    /// # Returns
    ///
    /// A tuple of (parsed response, stats) extracted from the API response
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - API request fails (network, timeout, rate limit)
    /// - Response cannot be parsed as valid JSON (including truncated responses)
    async fn send_and_parse<T: serde::de::DeserializeOwned>(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<(T, AiStats)> {
        use backon::Retryable;
        use tracing::warn;

        use crate::error::AptuError;
        use crate::retry::{is_retryable_anyhow, retry_backoff};

        // Check circuit breaker before attempting request
        if let Some(cb) = self.circuit_breaker()
            && cb.is_open()
        {
            return Err(AptuError::CircuitOpen.into());
        }

        // Start timing (outside retry loop to measure total time including retries)
        let start = std::time::Instant::now();

        let (parsed, completion): (T, ChatCompletionResponse) = (|| async {
            // Send HTTP request
            let completion = self.send_request_inner(request).await?;

            // Extract message content
            let content = completion
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .context("No response from AI model")?;

            debug!(response_length = content.len(), "Received AI response");

            // Parse JSON response (inside retry loop, so truncated responses are retried)
            let parsed: T = parse_ai_json(&content, self.name())?;

            Ok((parsed, completion))
        })
        .retry(retry_backoff())
        .when(is_retryable_anyhow)
        .notify(|err, dur| warn!(error = %err, delay = ?dur, "Retrying after error"))
        .await?;

        // Record success in circuit breaker
        if let Some(cb) = self.circuit_breaker() {
            cb.record_success();
        }

        // Calculate duration (total time including any retries)
        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;

        // Build AI stats from usage info (trust API's cost field)
        let (input_tokens, output_tokens, cost_usd) = if let Some(usage) = completion.usage {
            (usage.prompt_tokens, usage.completion_tokens, usage.cost)
        } else {
            // If no usage info, default to 0
            debug!("No usage information in API response");
            (0, 0, None)
        };

        let ai_stats = AiStats {
            model: self.model().to_string(),
            input_tokens,
            output_tokens,
            duration_ms,
            cost_usd,
            fallback_provider: None,
        };

        Ok((parsed, ai_stats))
    }

    /// Analyzes a GitHub issue using the provider's API.
    ///
    /// Returns a structured triage response with summary, labels, questions, duplicates, and usage stats.
    ///
    /// # Arguments
    ///
    /// * `issue` - Issue details to analyze
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - API request fails (network, timeout, rate limit)
    /// - Response cannot be parsed as valid JSON
    #[instrument(skip(self, issue), fields(issue_number = issue.number, repo = %format!("{}/{}", issue.owner, issue.repo)))]
    async fn analyze_issue(&self, issue: &IssueDetails) -> Result<AiResponse> {
        debug!(model = %self.model(), "Calling {} API", self.name());

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_system_prompt(None),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_user_prompt(issue),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Send request and parse JSON with retry logic
        let (triage, ai_stats) = self.send_and_parse::<TriageResponse>(&request).await?;

        debug!(
            input_tokens = ai_stats.input_tokens,
            output_tokens = ai_stats.output_tokens,
            duration_ms = ai_stats.duration_ms,
            cost_usd = ?ai_stats.cost_usd,
            "AI analysis complete"
        );

        Ok(AiResponse {
            triage,
            stats: ai_stats,
        })
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
    #[instrument(skip(self), fields(repo = %repo))]
    async fn create_issue(
        &self,
        title: &str,
        body: &str,
        repo: &str,
    ) -> Result<(super::types::CreateIssueResponse, AiStats)> {
        debug!(model = %self.model(), "Calling {} API for issue creation", self.name());

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_create_system_prompt(None),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_create_user_prompt(title, body, repo),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Send request and parse JSON with retry logic
        let (create_response, ai_stats) = self
            .send_and_parse::<super::types::CreateIssueResponse>(&request)
            .await?;

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

    /// Builds the system prompt for issue triage.
    #[must_use]
    fn build_system_prompt(custom_guidance: Option<&str>) -> String {
        let context = super::context::load_custom_guidance(custom_guidance);
        let schema = "{\n  \"summary\": \"A 2-3 sentence summary of what the issue is about and its impact\",\n  \"suggested_labels\": [\"label1\", \"label2\"],\n  \"clarifying_questions\": [\"question1\", \"question2\"],\n  \"potential_duplicates\": [\"#123\", \"#456\"],\n  \"related_issues\": [\n    {\n      \"number\": 789,\n      \"title\": \"Related issue title\",\n      \"reason\": \"Brief explanation of why this is related\"\n    }\n  ],\n  \"status_note\": \"Optional note about issue status (e.g., claimed, in-progress)\",\n  \"contributor_guidance\": {\n    \"beginner_friendly\": true,\n    \"reasoning\": \"1-2 sentence explanation of beginner-friendliness assessment\"\n  },\n  \"implementation_approach\": \"Optional suggestions for implementation based on repository structure\",\n  \"suggested_milestone\": \"Optional milestone title for the issue\"\n}";
        let guidelines = "Guidelines:\n\
- summary: Concise explanation of the problem/request and why it matters\n\
- suggested_labels: Prefer labels from the Available Labels list provided. Choose from: bug, enhancement, documentation, question, duplicate, invalid, wontfix. If a more specific label exists in the repository, use it instead of generic ones.\n\
- clarifying_questions: Only include if the issue lacks critical information. Leave empty array if issue is clear. Skip questions already answered in comments.\n\
- potential_duplicates: Only include if you detect likely duplicates from the context. Leave empty array if none. A duplicate is an issue that describes the exact same problem.\n\
- related_issues: Include issues from the search results that are contextually related but NOT duplicates. Provide brief reasoning for each. Leave empty array if none are relevant.\n\
- status_note: Detect if someone has claimed the issue or is working on it. Look for patterns like \"I'd like to work on this\", \"I'll submit a PR\", \"working on this\", or \"@user I've assigned you\". If claimed, set status_note to a brief description (e.g., \"Issue claimed by @username\"). If not claimed, leave as null or empty string.\n\
- contributor_guidance: Assess whether the issue is suitable for beginners. Consider: scope (small, well-defined), file count (few files to modify), required knowledge (no deep expertise needed), clarity (clear problem statement). Set beginner_friendly to true if all factors are favorable. Provide 1-2 sentence reasoning explaining the assessment.\n\
- implementation_approach: Based on the repository structure provided, suggest specific files or modules to modify. Reference the file paths from the repository structure. Be concrete and actionable. Leave as null or empty string if no specific guidance can be provided.\n\
- suggested_milestone: If applicable, suggest a milestone title from the Available Milestones list. Only include if a milestone is clearly relevant to the issue. Leave as null or empty string if no milestone is appropriate.\n\
\n\
Be helpful, concise, and actionable. Focus on what a maintainer needs to know.";
        format!(
            "You are an OSS issue triage assistant. Analyze the provided GitHub issue and \
             provide structured triage information.\n\n{context}\n\nYour response MUST be valid \
             JSON with this exact schema:\n{schema}\n\n{guidelines}"
        )
    }

    /// Builds the user prompt containing the issue details.
    #[must_use]
    fn build_user_prompt(issue: &IssueDetails) -> String {
        use std::fmt::Write;

        let mut prompt = String::new();

        prompt.push_str("<issue_content>\n");
        let _ = writeln!(prompt, "Title: {}\n", issue.title);

        // Truncate body if too long
        let body = if issue.body.len() > MAX_BODY_LENGTH {
            format!(
                "{}...\n[Body truncated - original length: {} chars]",
                &issue.body[..MAX_BODY_LENGTH],
                issue.body.len()
            )
        } else if issue.body.is_empty() {
            "[No description provided]".to_string()
        } else {
            issue.body.clone()
        };
        let _ = writeln!(prompt, "Body:\n{body}\n");

        // Include existing labels
        if !issue.labels.is_empty() {
            let _ = writeln!(prompt, "Existing Labels: {}\n", issue.labels.join(", "));
        }

        // Include recent comments (limited)
        if !issue.comments.is_empty() {
            prompt.push_str("Recent Comments:\n");
            for comment in issue.comments.iter().take(MAX_COMMENTS) {
                let comment_body = if comment.body.len() > 500 {
                    format!("{}...", &comment.body[..500])
                } else {
                    comment.body.clone()
                };
                let _ = writeln!(prompt, "- @{}: {}", comment.author, comment_body);
            }
            prompt.push('\n');
        }

        // Include related issues from search (for context)
        if !issue.repo_context.is_empty() {
            prompt.push_str("Related Issues in Repository (for context):\n");
            for related in issue.repo_context.iter().take(10) {
                let _ = writeln!(
                    prompt,
                    "- #{} [{}] {}",
                    related.number, related.state, related.title
                );
            }
            prompt.push('\n');
        }

        // Include repository structure (source files)
        if !issue.repo_tree.is_empty() {
            prompt.push_str("Repository Structure (source files):\n");
            for path in issue.repo_tree.iter().take(20) {
                let _ = writeln!(prompt, "- {path}");
            }
            prompt.push('\n');
        }

        // Include available labels
        if !issue.available_labels.is_empty() {
            prompt.push_str("Available Labels:\n");
            for label in issue.available_labels.iter().take(MAX_LABELS) {
                let description = if label.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", label.description)
                };
                let _ = writeln!(
                    prompt,
                    "- {} (color: #{}){}",
                    label.name, label.color, description
                );
            }
            prompt.push('\n');
        }

        // Include available milestones
        if !issue.available_milestones.is_empty() {
            prompt.push_str("Available Milestones:\n");
            for milestone in issue.available_milestones.iter().take(MAX_MILESTONES) {
                let description = if milestone.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", milestone.description)
                };
                let _ = writeln!(prompt, "- {}{}", milestone.title, description);
            }
            prompt.push('\n');
        }

        prompt.push_str("</issue_content>");

        prompt
    }

    /// Builds the system prompt for issue creation/formatting.
    #[must_use]
    fn build_create_system_prompt(custom_guidance: Option<&str>) -> String {
        let context = super::context::load_custom_guidance(custom_guidance);
        format!(
            r#"You are a GitHub issue formatting assistant. Your job is to take a raw issue title and body from a user and format them professionally for a GitHub repository.

{context}

Your response MUST be valid JSON with this exact schema:
{{
  "formatted_title": "Well-formatted issue title following conventional commit style",
  "formatted_body": "Professionally formatted issue body with clear sections",
  "suggested_labels": ["label1", "label2"]
}}

Guidelines:
- formatted_title: Use conventional commit style (e.g., "feat: add search functionality", "fix: resolve memory leak in parser"). Keep it concise (under 72 characters). No period at the end.
- formatted_body: Structure the body with clear sections:
  * Start with a brief 1-2 sentence summary if not already present
  * Use markdown formatting with headers (## Summary, ## Details, ## Steps to Reproduce, ## Expected Behavior, ## Actual Behavior, ## Context, etc.)
  * Keep sentences clear and concise
  * Use bullet points for lists
  * Improve grammar and clarity
  * Add relevant context if missing
- suggested_labels: Suggest up to 3 relevant GitHub labels. Common ones: bug, enhancement, documentation, question, duplicate, invalid, wontfix. Choose based on the issue content.

Be professional but friendly. Maintain the user's intent while improving clarity and structure."#
        )
    }

    /// Builds the user prompt for issue creation/formatting.
    #[must_use]
    fn build_create_user_prompt(title: &str, body: &str, _repo: &str) -> String {
        format!("Please format this GitHub issue:\n\nTitle: {title}\n\nBody:\n{body}")
    }

    /// Reviews a pull request using the provider's API.
    ///
    /// Analyzes PR metadata and file diffs to provide structured review feedback.
    ///
    /// # Arguments
    ///
    /// * `pr` - Pull request details including files and diffs
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - API request fails (network, timeout, rate limit)
    /// - Response cannot be parsed as valid JSON
    #[instrument(skip(self, pr), fields(pr_number = pr.number, repo = %format!("{}/{}", pr.owner, pr.repo)))]
    async fn review_pr(
        &self,
        pr: &super::types::PrDetails,
    ) -> Result<(super::types::PrReviewResponse, AiStats)> {
        debug!(model = %self.model(), "Calling {} API for PR review", self.name());

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_pr_review_system_prompt(None),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_pr_review_user_prompt(pr),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Send request and parse JSON with retry logic
        let (review, ai_stats) = self
            .send_and_parse::<super::types::PrReviewResponse>(&request)
            .await?;

        debug!(
            verdict = %review.verdict,
            input_tokens = ai_stats.input_tokens,
            output_tokens = ai_stats.output_tokens,
            duration_ms = ai_stats.duration_ms,
            "PR review complete with stats"
        );

        Ok((review, ai_stats))
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
    #[instrument(skip(self), fields(title = %title))]
    async fn suggest_pr_labels(
        &self,
        title: &str,
        body: &str,
        file_paths: &[String],
    ) -> Result<(Vec<String>, AiStats)> {
        debug!(model = %self.model(), "Calling {} API for PR label suggestion", self.name());

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_pr_label_system_prompt(None),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_pr_label_user_prompt(title, body, file_paths),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Send request and parse JSON with retry logic
        let (response, ai_stats) = self
            .send_and_parse::<super::types::PrLabelResponse>(&request)
            .await?;

        debug!(
            label_count = response.suggested_labels.len(),
            input_tokens = ai_stats.input_tokens,
            output_tokens = ai_stats.output_tokens,
            duration_ms = ai_stats.duration_ms,
            "PR label suggestion complete with stats"
        );

        Ok((response.suggested_labels, ai_stats))
    }

    /// Builds the system prompt for PR review.
    #[must_use]
    fn build_pr_review_system_prompt(custom_guidance: Option<&str>) -> String {
        let context = super::context::load_custom_guidance(custom_guidance);
        format!(
            r#"You are a code review assistant. Analyze the provided pull request and provide structured review feedback.

{context}

Your response MUST be valid JSON with this exact schema:
{{
  "summary": "A 2-3 sentence summary of what the PR does and its impact",
  "verdict": "approve|request_changes|comment",
  "strengths": ["strength1", "strength2"],
  "concerns": ["concern1", "concern2"],
  "comments": [
    {{
      "file": "path/to/file.rs",
      "line": 42,
      "comment": "Specific feedback about this line",
      "severity": "info|suggestion|warning|issue"
    }}
  ],
  "suggestions": ["suggestion1", "suggestion2"]
}}

Guidelines:
- summary: Concise explanation of the changes and their purpose
- verdict: Use "approve" for good PRs, "request_changes" for blocking issues, "comment" for feedback without blocking
- strengths: What the PR does well (good patterns, clear code, etc.)
- concerns: Potential issues or risks (bugs, performance, security, maintainability)
- comments: Specific line-level feedback. Use severity:
  - "info": Informational, no action needed
  - "suggestion": Optional improvement
  - "warning": Should consider changing
  - "issue": Should be fixed before merge
- suggestions: General improvements that are not blocking

Focus on:
1. Correctness: Does the code do what it claims?
2. Security: Any potential vulnerabilities?
3. Performance: Any obvious inefficiencies?
4. Maintainability: Is the code clear and well-structured?
5. Testing: Are changes adequately tested?

Be constructive and specific. Explain why something is an issue and how to fix it."#
        )
    }

    /// Builds the user prompt for PR review.
    #[must_use]
    fn build_pr_review_user_prompt(pr: &super::types::PrDetails) -> String {
        use std::fmt::Write;

        let mut prompt = String::new();

        prompt.push_str("<pull_request>\n");
        let _ = writeln!(prompt, "Title: {}\n", pr.title);
        let _ = writeln!(prompt, "Branch: {} -> {}\n", pr.head_branch, pr.base_branch);

        // PR description
        let body = if pr.body.is_empty() {
            "[No description provided]".to_string()
        } else if pr.body.len() > MAX_BODY_LENGTH {
            format!(
                "{}...\n[Description truncated - original length: {} chars]",
                &pr.body[..MAX_BODY_LENGTH],
                pr.body.len()
            )
        } else {
            pr.body.clone()
        };
        let _ = writeln!(prompt, "Description:\n{body}\n");

        // File changes with limits
        prompt.push_str("Files Changed:\n");
        let mut total_diff_size = 0;
        let mut files_included = 0;
        let mut files_skipped = 0;

        for file in &pr.files {
            // Check file count limit
            if files_included >= MAX_FILES {
                files_skipped += 1;
                continue;
            }

            let _ = writeln!(
                prompt,
                "- {} ({}) +{} -{}\n",
                file.filename, file.status, file.additions, file.deletions
            );

            // Include patch if available (truncate large patches)
            if let Some(patch) = &file.patch {
                const MAX_PATCH_LENGTH: usize = 2000;
                let patch_content = if patch.len() > MAX_PATCH_LENGTH {
                    format!(
                        "{}...\n[Patch truncated - original length: {} chars]",
                        &patch[..MAX_PATCH_LENGTH],
                        patch.len()
                    )
                } else {
                    patch.clone()
                };

                // Check if adding this patch would exceed total diff size limit
                let patch_size = patch_content.len();
                if total_diff_size + patch_size > MAX_TOTAL_DIFF_SIZE {
                    let _ = writeln!(
                        prompt,
                        "```diff\n[Patch omitted - total diff size limit reached]\n```\n"
                    );
                    files_skipped += 1;
                    continue;
                }

                let _ = writeln!(prompt, "```diff\n{patch_content}\n```\n");
                total_diff_size += patch_size;
            }

            files_included += 1;
        }

        // Add truncation message if files were skipped
        if files_skipped > 0 {
            let _ = writeln!(
                prompt,
                "\n[{files_skipped} files omitted due to size limits (MAX_FILES={MAX_FILES}, MAX_TOTAL_DIFF_SIZE={MAX_TOTAL_DIFF_SIZE})]"
            );
        }

        prompt.push_str("</pull_request>");

        prompt
    }

    /// Builds the system prompt for PR label suggestion.
    #[must_use]
    fn build_pr_label_system_prompt(custom_guidance: Option<&str>) -> String {
        let context = super::context::load_custom_guidance(custom_guidance);
        format!(
            r#"You are a GitHub label suggestion assistant. Analyze the provided pull request and suggest relevant labels.

{context}

Your response MUST be valid JSON with this exact schema:
{{
  "suggested_labels": ["label1", "label2", "label3"]
}}

Response format: json_object

Guidelines:
- suggested_labels: Suggest 1-3 relevant GitHub labels based on the PR content. Common labels include: bug, enhancement, documentation, feature, refactor, performance, security, testing, ci, dependencies. Choose labels that best describe the type of change.
- Focus on the PR title, description, and file paths to determine appropriate labels.
- Prefer specific labels over generic ones when possible.
- Only suggest labels that are commonly used in GitHub repositories.

Be concise and practical."#
        )
    }

    /// Builds the user prompt for PR label suggestion.
    #[must_use]
    fn build_pr_label_user_prompt(title: &str, body: &str, file_paths: &[String]) -> String {
        use std::fmt::Write;

        let mut prompt = String::new();

        prompt.push_str("<pull_request>\n");
        let _ = writeln!(prompt, "Title: {title}\n");

        // PR description
        let body_content = if body.is_empty() {
            "[No description provided]".to_string()
        } else if body.len() > MAX_BODY_LENGTH {
            format!(
                "{}...\n[Description truncated - original length: {} chars]",
                &body[..MAX_BODY_LENGTH],
                body.len()
            )
        } else {
            body.to_string()
        };
        let _ = writeln!(prompt, "Description:\n{body_content}\n");

        // File paths
        if !file_paths.is_empty() {
            prompt.push_str("Files Changed:\n");
            for path in file_paths.iter().take(20) {
                let _ = writeln!(prompt, "- {path}");
            }
            if file_paths.len() > 20 {
                let _ = writeln!(prompt, "- ... and {} more files", file_paths.len() - 20);
            }
            prompt.push('\n');
        }

        prompt.push_str("</pull_request>");

        prompt
    }

    /// Generate release notes from PR summaries.
    ///
    /// # Arguments
    ///
    /// * `prs` - List of PR summaries to synthesize
    /// * `version` - Version being released
    ///
    /// # Returns
    ///
    /// Structured release notes with theme, highlights, and categorized changes.
    #[instrument(skip(self, prs))]
    async fn generate_release_notes(
        &self,
        prs: Vec<super::types::PrSummary>,
        version: &str,
    ) -> Result<(super::types::ReleaseNotesResponse, AiStats)> {
        let prompt = Self::build_release_notes_prompt(&prs, version);
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            temperature: Some(0.7),
            max_tokens: Some(self.max_tokens()),
        };

        let (parsed, ai_stats) = self
            .send_and_parse::<super::types::ReleaseNotesResponse>(&request)
            .await?;

        debug!(
            input_tokens = ai_stats.input_tokens,
            output_tokens = ai_stats.output_tokens,
            duration_ms = ai_stats.duration_ms,
            "Release notes generation complete with stats"
        );

        Ok((parsed, ai_stats))
    }

    /// Build the user prompt for release notes generation.
    #[must_use]
    fn build_release_notes_prompt(prs: &[super::types::PrSummary], version: &str) -> String {
        let pr_list = prs
            .iter()
            .map(|pr| {
                format!(
                    "- #{}: {} (by @{})\n  {}",
                    pr.number,
                    pr.title,
                    pr.author,
                    pr.body.lines().next().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Generate release notes for version {version} based on these merged PRs:

{pr_list}

Create a curated release notes document with:
1. A theme/title that captures the essence of this release
2. A 1-2 sentence narrative about the release
3. 3-5 highlighted features
4. Categorized changes: Features, Fixes, Improvements, Documentation, Maintenance
5. List of contributors

Follow these conventions:
- No emojis
- Bold feature names with dash separator
- Include PR numbers in parentheses
- Group by user impact, not just commit type
- Filter CI/deps under Maintenance

Your response MUST be valid JSON with this exact schema:
{{
  "theme": "Release theme title",
  "narrative": "1-2 sentence summary",
  "highlights": ["highlight1", "highlight2"],
  "features": ["feature1", "feature2"],
  "fixes": ["fix1", "fix2"],
  "improvements": ["improvement1"],
  "documentation": ["doc change1"],
  "maintenance": ["maintenance1"],
  "contributors": ["@author1", "@author2"]
}}"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;

    impl AiProvider for TestProvider {
        fn name(&self) -> &str {
            "test"
        }

        fn api_url(&self) -> &str {
            "https://test.example.com"
        }

        fn api_key_env(&self) -> &str {
            "TEST_API_KEY"
        }

        fn http_client(&self) -> &Client {
            unimplemented!()
        }

        fn api_key(&self) -> &SecretString {
            unimplemented!()
        }

        fn model(&self) -> &str {
            "test-model"
        }

        fn max_tokens(&self) -> u32 {
            2048
        }

        fn temperature(&self) -> f32 {
            0.3
        }
    }

    #[test]
    fn test_build_system_prompt_contains_json_schema() {
        let prompt = TestProvider::build_system_prompt(None);
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("suggested_labels"));
        assert!(prompt.contains("clarifying_questions"));
        assert!(prompt.contains("potential_duplicates"));
        assert!(prompt.contains("status_note"));
    }

    #[test]
    fn test_build_user_prompt_with_delimiters() {
        let issue = IssueDetails::builder()
            .owner("test".to_string())
            .repo("repo".to_string())
            .number(1)
            .title("Test issue".to_string())
            .body("This is the body".to_string())
            .labels(vec!["bug".to_string()])
            .comments(vec![])
            .url("https://github.com/test/repo/issues/1".to_string())
            .build();

        let prompt = TestProvider::build_user_prompt(&issue);
        assert!(prompt.starts_with("<issue_content>"));
        assert!(prompt.ends_with("</issue_content>"));
        assert!(prompt.contains("Title: Test issue"));
        assert!(prompt.contains("This is the body"));
        assert!(prompt.contains("Existing Labels: bug"));
    }

    #[test]
    fn test_build_user_prompt_truncates_long_body() {
        let long_body = "x".repeat(5000);
        let issue = IssueDetails::builder()
            .owner("test".to_string())
            .repo("repo".to_string())
            .number(1)
            .title("Test".to_string())
            .body(long_body)
            .labels(vec![])
            .comments(vec![])
            .url("https://github.com/test/repo/issues/1".to_string())
            .build();

        let prompt = TestProvider::build_user_prompt(&issue);
        assert!(prompt.contains("[Body truncated"));
        assert!(prompt.contains("5000 chars"));
    }

    #[test]
    fn test_build_user_prompt_empty_body() {
        let issue = IssueDetails::builder()
            .owner("test".to_string())
            .repo("repo".to_string())
            .number(1)
            .title("Test".to_string())
            .body(String::new())
            .labels(vec![])
            .comments(vec![])
            .url("https://github.com/test/repo/issues/1".to_string())
            .build();

        let prompt = TestProvider::build_user_prompt(&issue);
        assert!(prompt.contains("[No description provided]"));
    }

    #[test]
    fn test_build_create_system_prompt_contains_json_schema() {
        let prompt = TestProvider::build_create_system_prompt(None);
        assert!(prompt.contains("formatted_title"));
        assert!(prompt.contains("formatted_body"));
        assert!(prompt.contains("suggested_labels"));
    }

    #[test]
    fn test_build_pr_review_user_prompt_respects_file_limit() {
        use super::super::types::{PrDetails, PrFile};

        let mut files = Vec::new();
        for i in 0..25 {
            files.push(PrFile {
                filename: format!("file{}.rs", i),
                status: "modified".to_string(),
                additions: 10,
                deletions: 5,
                patch: Some(format!("patch content {}", i)),
            });
        }

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
        };

        let prompt = TestProvider::build_pr_review_user_prompt(&pr);
        assert!(prompt.contains("files omitted due to size limits"));
        assert!(prompt.contains("MAX_FILES=20"));
    }

    #[test]
    fn test_build_pr_review_user_prompt_respects_diff_size_limit() {
        use super::super::types::{PrDetails, PrFile};

        // Create patches that will exceed the limit when combined
        // Each patch is ~30KB, so two will exceed 50KB limit
        let patch1 = "x".repeat(30_000);
        let patch2 = "y".repeat(30_000);

        let files = vec![
            PrFile {
                filename: "file1.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 50,
                patch: Some(patch1),
            },
            PrFile {
                filename: "file2.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 50,
                patch: Some(patch2),
            },
        ];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
        };

        let prompt = TestProvider::build_pr_review_user_prompt(&pr);
        // Both files should be listed
        assert!(prompt.contains("file1.rs"));
        assert!(prompt.contains("file2.rs"));
        // The second patch should be limited - verify the prompt doesn't contain both full patches
        // by checking that the total size is less than what two full 30KB patches would be
        assert!(prompt.len() < 65_000);
    }

    #[test]
    fn test_build_pr_review_user_prompt_with_no_patches() {
        use super::super::types::{PrDetails, PrFile};

        let files = vec![PrFile {
            filename: "file1.rs".to_string(),
            status: "added".to_string(),
            additions: 10,
            deletions: 0,
            patch: None,
        }];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
        };

        let prompt = TestProvider::build_pr_review_user_prompt(&pr);
        assert!(prompt.contains("file1.rs"));
        assert!(prompt.contains("added"));
        assert!(!prompt.contains("files omitted"));
    }

    #[test]
    fn test_build_pr_label_system_prompt_contains_json_schema() {
        let prompt = TestProvider::build_pr_label_system_prompt(None);
        assert!(prompt.contains("suggested_labels"));
        assert!(prompt.contains("json_object"));
        assert!(prompt.contains("bug"));
        assert!(prompt.contains("enhancement"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_with_title_and_body() {
        let title = "feat: add new feature";
        let body = "This PR adds a new feature";
        let files = vec!["src/main.rs".to_string(), "tests/test.rs".to_string()];

        let prompt = TestProvider::build_pr_label_user_prompt(title, body, &files);
        assert!(prompt.starts_with("<pull_request>"));
        assert!(prompt.ends_with("</pull_request>"));
        assert!(prompt.contains("feat: add new feature"));
        assert!(prompt.contains("This PR adds a new feature"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("tests/test.rs"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_empty_body() {
        let title = "fix: bug fix";
        let body = "";
        let files = vec!["src/lib.rs".to_string()];

        let prompt = TestProvider::build_pr_label_user_prompt(title, body, &files);
        assert!(prompt.contains("[No description provided]"));
        assert!(prompt.contains("src/lib.rs"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_truncates_long_body() {
        let title = "test";
        let long_body = "x".repeat(5000);
        let files = vec![];

        let prompt = TestProvider::build_pr_label_user_prompt(title, &long_body, &files);
        assert!(prompt.contains("[Description truncated"));
        assert!(prompt.contains("5000 chars"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_respects_file_limit() {
        let title = "test";
        let body = "test";
        let mut files = Vec::new();
        for i in 0..25 {
            files.push(format!("file{}.rs", i));
        }

        let prompt = TestProvider::build_pr_label_user_prompt(title, body, &files);
        assert!(prompt.contains("file0.rs"));
        assert!(prompt.contains("file19.rs"));
        assert!(!prompt.contains("file20.rs"));
        assert!(prompt.contains("... and 5 more files"));
    }

    #[test]
    fn test_build_pr_label_user_prompt_empty_files() {
        let title = "test";
        let body = "test";
        let files: Vec<String> = vec![];

        let prompt = TestProvider::build_pr_label_user_prompt(title, body, &files);
        assert!(prompt.contains("Title: test"));
        assert!(prompt.contains("Description:\ntest"));
        assert!(!prompt.contains("Files Changed:"));
    }

    #[test]
    fn test_parse_ai_json_with_valid_json() {
        #[derive(serde::Deserialize)]
        struct TestResponse {
            message: String,
        }

        let json = r#"{"message": "hello"}"#;
        let result: Result<TestResponse> = parse_ai_json(json, "test-provider");
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.message, "hello");
    }

    #[test]
    fn test_parse_ai_json_with_truncated_json() {
        #[derive(Debug, serde::Deserialize)]
        struct TestResponse {
            message: String,
        }

        let json = r#"{"message": "hello"#;
        let result: Result<TestResponse> = parse_ai_json(json, "test-provider");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("Truncated response from test-provider")
        );
    }

    #[test]
    fn test_parse_ai_json_with_malformed_json() {
        #[derive(Debug, serde::Deserialize)]
        struct TestResponse {
            message: String,
        }

        let json = r#"{"message": invalid}"#;
        let result: Result<TestResponse> = parse_ai_json(json, "test-provider");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid JSON response from AI"));
    }
}
