//! `OpenRouter` API client for AI-assisted issue triage.
//!
//! Provides functionality to analyze GitHub issues using the `OpenRouter` API
//! with structured JSON output.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{debug, instrument, warn};

use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, IssueDetails, ResponseFormat,
    TriageResponse,
};
use super::{OPENROUTER_API_KEY_ENV, OPENROUTER_API_URL};
use crate::config::AiConfig;

/// Maximum length for issue body to stay within token limits.
const MAX_BODY_LENGTH: usize = 4000;

/// Maximum number of comments to include in the prompt.
const MAX_COMMENTS: usize = 5;

/// Builds the system prompt for issue triage.
fn build_system_prompt() -> String {
    r##"You are an OSS issue triage assistant. Analyze the provided GitHub issue and provide structured triage information.

Your response MUST be valid JSON with this exact schema:
{
  "summary": "A 2-3 sentence summary of what the issue is about and its impact",
  "suggested_labels": ["label1", "label2"],
  "clarifying_questions": ["question1", "question2"],
  "potential_duplicates": ["#123", "#456"]
}

Guidelines:
- summary: Concise explanation of the problem/request and why it matters
- suggested_labels: Choose from: bug, enhancement, documentation, question, good first issue, help wanted, duplicate, invalid, wontfix
- clarifying_questions: Only include if the issue lacks critical information. Leave empty array if issue is clear.
- potential_duplicates: Only include if you detect likely duplicates from the context. Leave empty array if none.

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

    prompt.push_str("</issue_content>");

    prompt
}

/// Analyzes a GitHub issue using the `OpenRouter` API.
///
/// Returns a structured triage response with summary, labels, questions, and duplicates.
///
/// # Errors
///
/// Returns an error if:
/// - `OPENROUTER_API_KEY` environment variable is not set
/// - Model is not in free tier and `allow_paid_models` is false
/// - API request fails (network, timeout, rate limit)
/// - Response cannot be parsed as valid JSON
#[instrument(skip(config, issue), fields(issue_number = issue.number, repo = %format!("{}/{}", issue.owner, issue.repo)))]
pub async fn analyze_issue(config: &AiConfig, issue: &IssueDetails) -> Result<TriageResponse> {
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

    debug!(model = %config.model, "Calling OpenRouter API");

    // Build request
    let request = ChatCompletionRequest {
        model: config.model.clone(),
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

    // Create HTTP client with timeout
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .build()
        .context("Failed to create HTTP client")?;

    // Make API request
    let response = client
        .post(OPENROUTER_API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header(
            "HTTP-Referer",
            "https://github.com/clouatre-labs/project-aptu",
        )
        .header("X-Title", "Aptu CLI")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to OpenRouter API")?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();

        if status.as_u16() == 401 {
            anyhow::bail!(
                "Invalid OpenRouter API key. Check your {OPENROUTER_API_KEY_ENV} environment variable."
            );
        } else if status.as_u16() == 429 {
            warn!("Rate limited by OpenRouter API");
            anyhow::bail!(
                "OpenRouter rate limit exceeded. Please wait and try again.\n\
                 Consider upgrading your plan at: https://openrouter.ai/credits"
            );
        }
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

    Ok(triage)
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
        };

        let prompt = build_user_prompt(&issue);
        assert!(prompt.contains("[No description provided]"));
    }

    #[test]
    fn test_triage_response_parsing() {
        let json = r##"{
            "summary": "This is a test summary.",
            "suggested_labels": ["bug", "enhancement"],
            "clarifying_questions": ["What version?"],
            "potential_duplicates": ["#123"]
        }"##;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.summary, "This is a test summary.");
        assert_eq!(triage.suggested_labels, vec!["bug", "enhancement"]);
        assert_eq!(triage.clarifying_questions, vec!["What version?"]);
        assert_eq!(triage.potential_duplicates, vec!["#123"]);
    }

    #[test]
    fn test_triage_response_optional_fields() {
        let json = r#"{
            "summary": "Summary only.",
            "suggested_labels": ["bug"]
        }"#;

        let triage: TriageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(triage.summary, "Summary only.");
        assert!(triage.clarifying_questions.is_empty());
        assert!(triage.potential_duplicates.is_empty());
    }

    #[test]
    fn test_is_free_model() {
        use super::super::is_free_model;
        assert!(is_free_model("mistralai/devstral-2512:free"));
        assert!(is_free_model("google/gemini-2.0-flash-exp:free"));
        assert!(!is_free_model("openai/gpt-4"));
        assert!(!is_free_model("anthropic/claude-sonnet-4"));
    }
}
