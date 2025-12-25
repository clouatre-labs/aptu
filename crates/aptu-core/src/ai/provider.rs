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

/// Maximum length for issue body to stay within token limits.
pub const MAX_BODY_LENGTH: usize = 4000;

/// Maximum number of comments to include in the prompt.
pub const MAX_COMMENTS: usize = 5;

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

    /// Sends a chat completion request to the provider's API with retry logic.
    ///
    /// Default implementation handles HTTP headers, error responses (401, 429),
    /// and automatic retries with exponential backoff.
    async fn send_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        use backon::Retryable;
        use secrecy::ExposeSecret;
        use tracing::warn;

        use crate::error::AptuError;
        use crate::retry::{is_retryable_anyhow, retry_backoff};

        // Check circuit breaker before attempting request
        if let Some(cb) = self.circuit_breaker()
            && cb.is_open()
        {
            return Err(AptuError::CircuitOpen.into());
        }

        let completion: ChatCompletionResponse = (|| async {
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
        })
        .retry(retry_backoff())
        .when(is_retryable_anyhow)
        .notify(|err, dur| warn!(error = %err, delay = ?dur, "Retrying after error"))
        .await?;

        // Record success in circuit breaker
        if let Some(cb) = self.circuit_breaker() {
            cb.record_success();
        }

        Ok(completion)
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

        // Start timing (outside retry loop to measure total time including retries)
        let start = std::time::Instant::now();

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_user_prompt(issue),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Make API request with retry logic
        let completion = self.send_request(&request).await?;

        // Calculate duration (total time including any retries)
        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;

        // Extract message content
        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .context("No response from AI model")?;

        debug!(response_length = content.len(), "Received AI response");

        // Parse JSON response
        let triage: TriageResponse = serde_json::from_str(&content).with_context(|| {
            format!("Failed to parse AI response as JSON. Raw response:\n{content}")
        })?;

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
        };

        debug!(
            input_tokens,
            output_tokens,
            duration_ms,
            ?cost_usd,
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
    ) -> Result<super::types::CreateIssueResponse> {
        debug!(model = %self.model(), "Calling {} API for issue creation", self.name());

        // Start timing
        let start = std::time::Instant::now();

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_create_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_create_user_prompt(title, body, repo),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Make API request with retry logic
        let completion = self.send_request(&request).await?;

        // Extract message content
        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .context("No response from AI model")?;

        debug!(response_length = content.len(), "Received AI response");

        // Parse JSON response
        let create_response: super::types::CreateIssueResponse = serde_json::from_str(&content)
            .with_context(|| {
                format!("Failed to parse AI response as JSON. Raw response:\n{content}")
            })?;

        #[allow(clippy::cast_possible_truncation)]
        let _duration_ms = start.elapsed().as_millis() as u64;

        debug!(
            title_len = create_response.formatted_title.len(),
            body_len = create_response.formatted_body.len(),
            labels = create_response.suggested_labels.len(),
            "Issue formatting complete"
        );

        Ok(create_response)
    }

    /// Builds the system prompt for issue triage.
    #[must_use]
    fn build_system_prompt() -> String {
        r##"You are an OSS issue triage assistant. Analyze the provided GitHub issue and provide structured triage information.

Your response MUST be valid JSON with this exact schema:
{
  "summary": "A 2-3 sentence summary of what the issue is about and its impact",
  "suggested_labels": ["label1", "label2"],
  "clarifying_questions": ["question1", "question2"],
  "potential_duplicates": ["#123", "#456"],
  "related_issues": [
    {
      "number": 789,
      "title": "Related issue title",
      "reason": "Brief explanation of why this is related"
    }
  ],
  "status_note": "Optional note about issue status (e.g., claimed, in-progress)",
  "contributor_guidance": {
    "beginner_friendly": true,
    "reasoning": "1-2 sentence explanation of beginner-friendliness assessment"
  },
  "implementation_approach": "Optional suggestions for implementation based on repository structure",
  "suggested_milestone": "Optional milestone title for the issue"
}

Guidelines:
- summary: Concise explanation of the problem/request and why it matters
- suggested_labels: Prefer labels from the Available Labels list provided. Choose from: bug, enhancement, documentation, question, good first issue, help wanted, duplicate, invalid, wontfix. If a more specific label exists in the repository, use it instead of generic ones.
  - good first issue: Consider this label when the issue has: clear and well-defined scope, minimal codebase knowledge required, isolated change with minimal dependencies, good documentation or examples in the repository, no complex architectural understanding needed.
  - help wanted: Consider this label when the issue has: well-defined requirements and acceptance criteria, maintainer capacity is limited, issue is not blocked by other work, suitable for external contributors with domain knowledge.
- clarifying_questions: Only include if the issue lacks critical information. Leave empty array if issue is clear. Skip questions already answered in comments.
- potential_duplicates: Only include if you detect likely duplicates from the context. Leave empty array if none. A duplicate is an issue that describes the exact same problem.
- related_issues: Include issues from the search results that are contextually related but NOT duplicates. Provide brief reasoning for each. Leave empty array if none are relevant.
- status_note: Detect if someone has claimed the issue or is working on it. Look for patterns like "I'd like to work on this", "I'll submit a PR", "working on this", or "@user I've assigned you". If claimed, set status_note to a brief description (e.g., "Issue claimed by @username"). If not claimed, leave as null or empty string. IMPORTANT: If issue is claimed, do NOT suggest 'help wanted' label.
- contributor_guidance: Assess whether the issue is suitable for beginners. Consider: scope (small, well-defined), file count (few files to modify), required knowledge (no deep expertise needed), clarity (clear problem statement). Set beginner_friendly to true if all factors are favorable. Provide 1-2 sentence reasoning explaining the assessment.
- implementation_approach: Based on the repository structure provided, suggest specific files or modules to modify. Reference the file paths from the repository structure. Be concrete and actionable. Leave as null or empty string if no specific guidance can be provided.
- suggested_milestone: If applicable, suggest a milestone title from the Available Milestones list. Only include if a milestone is clearly relevant to the issue. Leave as null or empty string if no milestone is appropriate.

Be helpful, concise, and actionable. Focus on what a maintainer needs to know."##.to_string()
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
            for label in issue.available_labels.iter().take(30) {
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
            for milestone in issue.available_milestones.iter().take(10) {
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
    fn build_create_system_prompt() -> String {
        r#"You are a GitHub issue formatting assistant. Your job is to take a raw issue title and body from a user and format them professionally for a GitHub repository.

Your response MUST be valid JSON with this exact schema:
{
  "formatted_title": "Well-formatted issue title following conventional commit style",
  "formatted_body": "Professionally formatted issue body with clear sections",
  "suggested_labels": ["label1", "label2"]
}

Guidelines:
- formatted_title: Use conventional commit style (e.g., "feat: add search functionality", "fix: resolve memory leak in parser"). Keep it concise (under 72 characters). No period at the end.
- formatted_body: Structure the body with clear sections:
  * Start with a brief 1-2 sentence summary if not already present
  * Use markdown formatting with headers (## Summary, ## Details, ## Steps to Reproduce, ## Expected Behavior, ## Actual Behavior, ## Context, etc.)
  * Keep sentences clear and concise
  * Use bullet points for lists
  * Improve grammar and clarity
  * Add relevant context if missing
- suggested_labels: Suggest up to 3 relevant GitHub labels. Common ones: bug, enhancement, documentation, question, good first issue, help wanted, duplicate, invalid, wontfix. Choose based on the issue content.

Be professional but friendly. Maintain the user's intent while improving clarity and structure."#.to_string()
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
    ) -> Result<super::types::PrReviewResponse> {
        debug!(model = %self.model(), "Calling {} API for PR review", self.name());

        // Build request
        let request = ChatCompletionRequest {
            model: self.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_pr_review_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Self::build_pr_review_user_prompt(pr),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(self.max_tokens()),
            temperature: Some(self.temperature()),
        };

        // Make API request with retry logic
        let completion = self.send_request(&request).await?;

        // Extract message content
        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .context("No response from AI model")?;

        debug!(response_length = content.len(), "Received AI response");

        // Parse JSON response
        let review: super::types::PrReviewResponse =
            serde_json::from_str(&content).with_context(|| {
                format!("Failed to parse AI response as JSON. Raw response:\n{content}")
            })?;

        debug!(verdict = %review.verdict, "PR review complete");

        Ok(review)
    }

    /// Builds the system prompt for PR review.
    #[must_use]
    fn build_pr_review_system_prompt() -> String {
        r#"You are a code review assistant. Analyze the provided pull request and provide structured review feedback.

Your response MUST be valid JSON with this exact schema:
{
  "summary": "A 2-3 sentence summary of what the PR does and its impact",
  "verdict": "approve|request_changes|comment",
  "strengths": ["strength1", "strength2"],
  "concerns": ["concern1", "concern2"],
  "comments": [
    {
      "file": "path/to/file.rs",
      "line": 42,
      "comment": "Specific feedback about this line",
      "severity": "info|suggestion|warning|issue"
    }
  ],
  "suggestions": ["suggestion1", "suggestion2"]
}

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

Be constructive and specific. Explain why something is an issue and how to fix it."#.to_string()
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

        // File changes
        prompt.push_str("Files Changed:\n");
        for file in &pr.files {
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
                let _ = writeln!(prompt, "```diff\n{patch_content}\n```\n");
            }
        }

        prompt.push_str("</pull_request>");

        prompt
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
        let prompt = TestProvider::build_system_prompt();
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("suggested_labels"));
        assert!(prompt.contains("clarifying_questions"));
        assert!(prompt.contains("potential_duplicates"));
        assert!(prompt.contains("status_note"));
    }

    #[test]
    fn test_build_user_prompt_with_delimiters() {
        let issue = IssueDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test issue".to_string(),
            body: "This is the body".to_string(),
            labels: vec!["bug".to_string()],
            comments: vec![],
            url: "https://github.com/test/repo/issues/1".to_string(),
            repo_context: Vec::new(),
            repo_tree: Vec::new(),
            available_labels: Vec::new(),
            available_milestones: Vec::new(),
            viewer_permission: None,
        };

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
        let issue = IssueDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test".to_string(),
            body: long_body,
            labels: vec![],
            comments: vec![],
            url: "https://github.com/test/repo/issues/1".to_string(),
            repo_context: Vec::new(),
            repo_tree: Vec::new(),
            available_labels: Vec::new(),
            available_milestones: Vec::new(),
            viewer_permission: None,
        };

        let prompt = TestProvider::build_user_prompt(&issue);
        assert!(prompt.contains("[Body truncated"));
        assert!(prompt.contains("5000 chars"));
    }

    #[test]
    fn test_build_user_prompt_empty_body() {
        let issue = IssueDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test".to_string(),
            body: String::new(),
            labels: vec![],
            comments: vec![],
            url: "https://github.com/test/repo/issues/1".to_string(),
            repo_context: Vec::new(),
            repo_tree: Vec::new(),
            available_labels: Vec::new(),
            available_milestones: Vec::new(),
            viewer_permission: None,
        };

        let prompt = TestProvider::build_user_prompt(&issue);
        assert!(prompt.contains("[No description provided]"));
    }

    #[test]
    fn test_build_create_system_prompt_contains_json_schema() {
        let prompt = TestProvider::build_create_system_prompt();
        assert!(prompt.contains("formatted_title"));
        assert!(prompt.contains("formatted_body"));
        assert!(prompt.contains("suggested_labels"));
    }
}
