// SPDX-License-Identifier: Apache-2.0

//! `OpenRouter` API client for AI-assisted issue triage.
//!
//! Provides functionality to analyze GitHub issues using the `OpenRouter` API
//! with structured JSON output.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use backon::Retryable;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, instrument, warn};

use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, IssueDetails, ResponseFormat,
    TriageResponse,
};
use super::{AiResponse, OPENROUTER_API_KEY_ENV, OPENROUTER_API_URL};
use crate::config::AiConfig;
use crate::error::AptuError;
use crate::history::AiStats;
use crate::retry::{is_retryable_anyhow, retry_backoff};

/// `OpenRouter` account credits status.
#[derive(Debug, Clone)]
pub struct CreditsStatus {
    /// Available credits in USD.
    pub credits: f64,
}

impl CreditsStatus {
    /// Returns a human-readable status message.
    #[must_use]
    pub fn message(&self) -> String {
        format!("OpenRouter credits: ${:.4}", self.credits)
    }
}

/// Maximum length for issue body to stay within token limits.
const MAX_BODY_LENGTH: usize = 4000;

/// Maximum number of comments to include in the prompt.
const MAX_COMMENTS: usize = 5;

/// `OpenRouter` API client for issue triage.
///
/// Holds HTTP client, API key, and model configuration for reuse across multiple requests.
/// Enables connection pooling and cleaner API.
pub struct OpenRouterClient {
    /// HTTP client with configured timeout.
    http: Client,
    /// API key for `OpenRouter` authentication.
    api_key: SecretString,
    /// Model name (e.g., "mistralai/devstral-2512:free").
    model: String,
}

impl OpenRouterClient {
    /// Creates a new `OpenRouter` client from configuration.
    ///
    /// Validates the model against cost control settings and fetches the API key
    /// from the environment.
    ///
    /// # Arguments
    ///
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model is not in free tier and `allow_paid_models` is false
    /// - `OPENROUTER_API_KEY` environment variable is not set
    /// - HTTP client creation fails
    pub fn new(config: &AiConfig) -> Result<Self> {
        // Validate model against cost control
        if !config.allow_paid_models && !super::is_free_model(&config.model) {
            anyhow::bail!(
                "Model '{}' is not in the free tier.\n\
                 To use paid models, set `allow_paid_models = true` in your config file:\n\
                 {}\n\n\
                 Or use a free model like: mistralai/devstral-2512:free",
                config.model,
                crate::config::config_file_path().display()
            );
        }

        // Get API key from environment
        let api_key = env::var(OPENROUTER_API_KEY_ENV).with_context(|| {
            format!(
                "Missing {OPENROUTER_API_KEY_ENV} environment variable.\n\
                 Set it with: export {OPENROUTER_API_KEY_ENV}=your_api_key\n\
                 Get a free key at: https://openrouter.ai/keys"
            )
        })?;

        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            api_key: SecretString::new(api_key.into()),
            model: config.model.clone(),
        })
    }

    /// Creates a new `OpenRouter` client with a provided API key.
    ///
    /// This constructor allows callers to provide an API key directly,
    /// enabling multi-platform credential resolution (e.g., from iOS keychain via FFI).
    ///
    /// # Arguments
    ///
    /// * `api_key` - `OpenRouter` API key as a `SecretString`
    /// * `config` - AI configuration with model, timeout, and cost control settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model is not in free tier and `allow_paid_models` is false
    /// - HTTP client creation fails
    pub fn with_api_key(api_key: SecretString, config: &AiConfig) -> Result<Self> {
        // Validate model against cost control
        if !config.allow_paid_models && !super::is_free_model(&config.model) {
            anyhow::bail!(
                "Model '{}' is not in the free tier.\n\
                 To use paid models, set `allow_paid_models = true` in your config file:\n\
                 {}\n\n\
                 Or use a free model like: mistralai/devstral-2512:free",
                config.model,
                crate::config::config_file_path().display()
            );
        }

        // Create HTTP client with timeout
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            api_key,
            model: config.model.clone(),
        })
    }

    /// Sends a chat completion request to the `OpenRouter` API with retry logic.
    ///
    /// Handles HTTP headers, error responses (401, 429), and automatic retries
    /// with exponential backoff.
    ///
    /// # Arguments
    ///
    /// * `request` - The chat completion request to send
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - API returns an error status code
    /// - Response cannot be parsed as JSON
    async fn send_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let completion: ChatCompletionResponse = (|| async {
            let response = self
                .http
                .post(OPENROUTER_API_URL)
                .header(
                    "Authorization",
                    format!("Bearer {}", self.api_key.expose_secret()),
                )
                .header("Content-Type", "application/json")
                .header(
                    "HTTP-Referer",
                    "https://github.com/clouatre-labs/project-aptu",
                )
                .header("X-Title", "Aptu CLI")
                .json(request)
                .send()
                .await
                .context("Failed to send request to OpenRouter API")?;

            // Check for HTTP errors
            let status = response.status();
            if !status.is_success() {
                if status.as_u16() == 401 {
                    anyhow::bail!(
                        "Invalid OpenRouter API key. Check your {OPENROUTER_API_KEY_ENV} environment variable."
                    );
                } else if status.as_u16() == 429 {
                    warn!("Rate limited by OpenRouter API");
                    // Parse Retry-After header (seconds), default to 0 if not present
                    let retry_after = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    debug!(retry_after, "Parsed Retry-After header");
                    return Err(AptuError::RateLimited {
                        provider: "openrouter".to_string(),
                        retry_after,
                    }
                    .into());
                }
                let error_body = response.text().await.unwrap_or_default();
                anyhow::bail!(
                    "OpenRouter API error (HTTP {}): {}",
                    status.as_u16(),
                    error_body
                );
            }

            // Parse response
            let completion: ChatCompletionResponse = response
                .json()
                .await
                .context("Failed to parse OpenRouter API response")?;

            Ok(completion)
        })
        .retry(retry_backoff())
        .when(is_retryable_anyhow)
        .notify(|err, dur| warn!(error = %err, delay = ?dur, "Retrying after error"))
        .await?;

        Ok(completion)
    }

    /// Analyzes a GitHub issue using the `OpenRouter` API.
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
    #[allow(clippy::too_many_lines)]
    pub async fn analyze_issue(&self, issue: &IssueDetails) -> Result<AiResponse> {
        debug!(model = %self.model, "Calling OpenRouter API");

        // Start timing (outside retry loop to measure total time including retries)
        let start = std::time::Instant::now();

        // Build request
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: build_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: build_user_prompt(issue),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(1024),
            temperature: Some(0.3),
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
            model: self.model.clone(),
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

    /// Creates a formatted GitHub issue using the `OpenRouter` API.
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
    pub async fn create_issue(
        &self,
        title: &str,
        body: &str,
        repo: &str,
    ) -> Result<super::types::CreateIssueResponse> {
        debug!(model = %self.model, "Calling OpenRouter API for issue creation");

        // Start timing
        let start = std::time::Instant::now();

        // Build request
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: build_create_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: build_create_user_prompt(title, body, repo),
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(1024),
            temperature: Some(0.3),
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
}

/// Builds the system prompt for issue triage.
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
fn build_create_user_prompt(title: &str, body: &str, _repo: &str) -> String {
    format!("Please format this GitHub issue:\n\nTitle: {title}\n\nBody:\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_prompt_contains_json_schema() {
        let prompt = build_system_prompt();
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("suggested_labels"));
        assert!(prompt.contains("clarifying_questions"));
        assert!(prompt.contains("potential_duplicates"));
        assert!(prompt.contains("status_note"));
    }

    #[test]
    fn test_build_system_prompt_contains_claim_detection_keywords() {
        let prompt = build_system_prompt();
        assert!(prompt.contains("claimed") || prompt.contains("working on"));
        assert!(prompt.contains("help wanted"));
    }

    #[test]
    fn test_triage_response_with_status_note() {
        let json = r#"{
            "summary": "Test summary",
            "suggested_labels": ["bug"],
            "status_note": "Issue claimed by @user"
        }"#;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            triage.status_note,
            Some("Issue claimed by @user".to_string())
        );
    }

    #[test]
    fn test_triage_response_without_status_note() {
        let json = r#"{
            "summary": "Test summary",
            "suggested_labels": ["bug"]
        }"#;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.status_note, None);
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

        let prompt = build_user_prompt(&issue);
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

        let prompt = build_user_prompt(&issue);
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

        let prompt = build_user_prompt(&issue);
        assert!(prompt.contains("[No description provided]"));
    }

    #[test]
    fn test_triage_response_full() {
        let json = r##"{
            "summary": "This is a test summary.",
            "suggested_labels": ["bug", "enhancement"],
            "clarifying_questions": ["What version?"],
            "potential_duplicates": ["#123"],
            "status_note": "Issue claimed by @user",
            "contributor_guidance": {
                "beginner_friendly": true,
                "reasoning": "Small scope, well-defined problem statement."
            }
        }"##;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.summary, "This is a test summary.");
        assert_eq!(triage.suggested_labels, vec!["bug", "enhancement"]);
        assert_eq!(triage.clarifying_questions, vec!["What version?"]);
        assert_eq!(triage.potential_duplicates, vec!["#123"]);
        assert_eq!(
            triage.status_note,
            Some("Issue claimed by @user".to_string())
        );
        assert!(triage.contributor_guidance.is_some());
        let guidance = triage.contributor_guidance.unwrap();
        assert!(guidance.beginner_friendly);
        assert_eq!(
            guidance.reasoning,
            "Small scope, well-defined problem statement."
        );
    }

    #[test]
    fn test_triage_response_minimal() {
        let json = r#"{
            "summary": "Summary only.",
            "suggested_labels": ["bug"]
        }"#;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.summary, "Summary only.");
        assert_eq!(triage.suggested_labels, vec!["bug"]);
        assert!(triage.clarifying_questions.is_empty());
        assert!(triage.potential_duplicates.is_empty());
        assert_eq!(triage.status_note, None);
        assert!(triage.contributor_guidance.is_none());
    }

    #[test]
    fn test_triage_response_partial() {
        let json = r#"{
            "summary": "Test summary",
            "suggested_labels": ["enhancement"],
            "contributor_guidance": {
                "beginner_friendly": false,
                "reasoning": "Requires deep knowledge of the compiler internals."
            }
        }"#;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.summary, "Test summary");
        assert_eq!(triage.suggested_labels, vec!["enhancement"]);
        assert!(triage.clarifying_questions.is_empty());
        assert!(triage.potential_duplicates.is_empty());
        assert_eq!(triage.status_note, None);
        assert!(triage.contributor_guidance.is_some());
        let guidance = triage.contributor_guidance.unwrap();
        assert!(!guidance.beginner_friendly);
        assert_eq!(
            guidance.reasoning,
            "Requires deep knowledge of the compiler internals."
        );
    }

    #[test]
    fn test_is_free_model() {
        use super::super::is_free_model;
        assert!(is_free_model("mistralai/devstral-2512:free"));
        assert!(is_free_model("google/gemini-2.0-flash-exp:free"));
        assert!(!is_free_model("openai/gpt-4"));
        assert!(!is_free_model("anthropic/claude-sonnet-4"));
    }

    #[test]
    fn test_build_system_prompt_contains_contributor_guidance() {
        let prompt = build_system_prompt();
        assert!(prompt.contains("contributor_guidance"));
        assert!(prompt.contains("beginner_friendly"));
        assert!(prompt.contains("reasoning"));
        assert!(prompt.contains("scope"));
        assert!(prompt.contains("file count"));
        assert!(prompt.contains("required knowledge"));
    }
}
