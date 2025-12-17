//! AI request/response types for `OpenRouter` API.
//!
//! Defines the structures used for communicating with the `OpenRouter` API
//! and parsing triage responses.

use serde::{Deserialize, Serialize};

/// A chat message for the `OpenRouter` API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: "system", "user", or "assistant".
    pub role: String,
    /// Message content.
    pub content: String,
}

/// Request body for `OpenRouter` chat completions API.
#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    /// Model identifier (e.g., "mistralai/devstral-2512:free").
    pub model: String,
    /// List of messages in the conversation.
    pub messages: Vec<ChatMessage>,
    /// Response format specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// Maximum tokens in response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature for response randomness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Response format specification for structured output.
#[derive(Debug, Serialize)]
pub struct ResponseFormat {
    /// Type of response format ("`json_object`" for structured output).
    #[serde(rename = "type")]
    pub format_type: String,
}

/// Response from `OpenRouter` chat completions API.
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    /// List of choices (usually just one).
    pub choices: Vec<Choice>,
}

/// A single choice in the chat completion response.
#[derive(Debug, Deserialize)]
pub struct Choice {
    /// The generated message.
    pub message: ChatMessage,
}

/// Structured triage response from AI.
///
/// This is the expected JSON structure in the AI's response content.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TriageResponse {
    /// 2-3 sentence summary of the issue.
    pub summary: String,
    /// Suggested labels for the issue.
    pub suggested_labels: Vec<String>,
    /// Clarifying questions for the issue reporter.
    #[serde(default)]
    pub clarifying_questions: Vec<String>,
    /// Potential duplicate issue numbers/references.
    #[serde(default)]
    pub potential_duplicates: Vec<String>,
}

/// Details about an issue for triage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDetails {
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Issue body (markdown content).
    pub body: String,
    /// Current labels on the issue.
    pub labels: Vec<String>,
    /// Recent comments on the issue.
    pub comments: Vec<IssueComment>,
    /// Issue URL.
    #[allow(dead_code)] // Used for future features (history tracking)
    pub url: String,
}

/// A comment on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    /// Comment author username.
    pub author: String,
    /// Comment body.
    pub body: String,
}
