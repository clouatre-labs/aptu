// SPDX-License-Identifier: Apache-2.0

//! AI request/response types for API communication.
//!
//! Defines the structures used for communicating with AI provider APIs
//! and parsing triage responses.

use serde::{Deserialize, Serialize};

/// Account credits status for `OpenRouter`.
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
    /// Type of response format ("`json_object`" or "`json_schema`" for structured output).
    #[serde(rename = "type")]
    pub format_type: String,
    /// JSON schema for structured output (optional, used with `json_schema` type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// Response from `OpenRouter` chat completions API.
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    /// List of choices (usually just one).
    pub choices: Vec<Choice>,
    /// Usage information from the API.
    #[serde(default)]
    pub usage: Option<UsageInfo>,
}

/// Token usage information from the API.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UsageInfo {
    /// Number of tokens in the prompt.
    #[serde(default)]
    pub prompt_tokens: u64,
    /// Number of tokens in the completion.
    #[serde(default)]
    pub completion_tokens: u64,
    /// Total tokens used.
    #[serde(default)]
    pub total_tokens: u64,
    /// Cost in USD (from `OpenRouter` API).
    #[serde(default)]
    pub cost: Option<f64>,
}

/// A single choice in the chat completion response.
#[derive(Debug, Deserialize)]
pub struct Choice {
    /// The generated message.
    pub message: ChatMessage,
}

/// Guidance for contributors on whether an issue is beginner-friendly.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContributorGuidance {
    /// Whether the issue is suitable for beginners.
    pub beginner_friendly: bool,
    /// Reasoning for the beginner-friendly assessment (1-2 sentences).
    pub reasoning: String,
}

/// A related issue found via search.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelatedIssue {
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Reason why this issue is related.
    pub reason: String,
}

/// Structured triage response from AI.
///
/// This is the expected JSON structure in the AI's response content.
///
/// # JSON Output
///
/// When using `--output json`, commands return this structure:
///
/// ```json
/// {
///   "summary": "Brief 2-3 sentence overview",
///   "suggested_labels": ["bug", "needs-triage"],
///   "clarifying_questions": ["What version?"],
///   "potential_duplicates": [123, 456],
///   "related_issues": [
///     {"number": 789, "title": "Similar issue", "reason": "Same component"}
///   ],
///   "contributor_guidance": {
///     "beginner_friendly": true,
///     "reasoning": "Well-scoped with clear requirements"
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    /// Related issues (not duplicates, but contextually relevant).
    #[serde(default)]
    pub related_issues: Vec<RelatedIssue>,
    /// Status note about the issue (e.g., if it's already claimed or in-progress).
    #[serde(default)]
    pub status_note: Option<String>,
    /// Guidance for contributors on beginner-friendliness.
    #[serde(default)]
    pub contributor_guidance: Option<ContributorGuidance>,
    /// Implementation approach suggestions based on repository structure.
    #[serde(default)]
    pub implementation_approach: Option<String>,
    /// Suggested milestone for the issue.
    #[serde(default)]
    pub suggested_milestone: Option<String>,
}

/// Context about a related issue from repository search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIssueContext {
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Issue labels.
    pub labels: Vec<String>,
    /// Issue state (open or closed).
    pub state: String,
}

/// A label available in the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoLabel {
    /// Label name.
    pub name: String,
    /// Label description.
    pub description: String,
    /// Label color (hex code).
    pub color: String,
}

/// A milestone available in the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMilestone {
    /// Milestone number.
    pub number: u64,
    /// Milestone title.
    pub title: String,
    /// Milestone description.
    pub description: String,
}

/// Details about an issue for triage.
#[derive(Debug, Clone, Serialize, Deserialize, bon::Builder)]
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
    #[builder(default)]
    pub labels: Vec<String>,
    /// Current milestone on the issue (if any).
    #[serde(default)]
    pub milestone: Option<String>,
    /// Recent comments on the issue.
    #[builder(default)]
    pub comments: Vec<IssueComment>,
    /// Issue URL.
    #[allow(dead_code)] // Used for future features (history tracking)
    pub url: String,
    /// Related issues from repository search (for AI context).
    #[serde(default)]
    #[builder(default)]
    pub repo_context: Vec<RepoIssueContext>,
    /// Repository file tree (source files for implementation context).
    #[serde(default)]
    #[builder(default)]
    pub repo_tree: Vec<String>,
    /// Available labels in the repository.
    #[serde(default)]
    #[builder(default)]
    pub available_labels: Vec<RepoLabel>,
    /// Available milestones in the repository.
    #[serde(default)]
    #[builder(default)]
    pub available_milestones: Vec<RepoMilestone>,
    /// Viewer permission level on the repository.
    #[serde(default)]
    pub viewer_permission: Option<String>,
    /// Issue author login.
    #[serde(default)]
    pub author: Option<String>,
    /// Issue creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Issue last update timestamp.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A comment on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    /// Comment author username.
    pub author: String,
    /// Comment body.
    pub body: String,
}

/// Response from AI for creating an issue.
///
/// Contains formatted issue content and suggested labels based on AI analysis.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateIssueResponse {
    /// Formatted issue title (follows conventional commit style).
    pub formatted_title: String,
    /// Formatted issue body with structured sections.
    pub formatted_body: String,
    /// Suggested labels for the issue.
    pub suggested_labels: Vec<String>,
}

/// Details about a pull request for AI review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetails {
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request number.
    pub number: u64,
    /// Pull request title.
    pub title: String,
    /// Pull request body/description.
    pub body: String,
    /// Base branch (target of the PR).
    pub base_branch: String,
    /// Head branch (source of the PR).
    pub head_branch: String,
    /// Files changed in the PR with their diffs.
    pub files: Vec<PrFile>,
    /// Pull request URL.
    pub url: String,
    /// Labels applied to the PR.
    #[serde(default)]
    pub labels: Vec<String>,
}

/// A file changed in a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrFile {
    /// File path.
    pub filename: String,
    /// Change status (added, modified, removed, renamed).
    pub status: String,
    /// Number of additions.
    pub additions: u64,
    /// Number of deletions.
    pub deletions: u64,
    /// Unified diff patch (may be truncated for large files).
    pub patch: Option<String>,
}

/// Severity level for PR review comments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentSeverity {
    /// Informational comment.
    Info,
    /// Suggested improvement.
    Suggestion,
    /// Warning about potential issues.
    Warning,
    /// Critical issue that should be addressed.
    Issue,
}

impl std::fmt::Display for CommentSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl CommentSeverity {
    /// Returns the severity level as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            CommentSeverity::Info => "info",
            CommentSeverity::Suggestion => "suggestion",
            CommentSeverity::Warning => "warning",
            CommentSeverity::Issue => "issue",
        }
    }
}

/// A specific comment on a line of code in a PR review.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrReviewComment {
    /// File path the comment applies to.
    pub file: String,
    /// Line number in the diff (optional for general file comments).
    pub line: Option<u32>,
    /// The comment text.
    pub comment: String,
    /// Severity level for the comment.
    pub severity: CommentSeverity,
}

/// Structured PR review response from AI.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PrReviewResponse {
    /// Overall summary of the PR (2-3 sentences).
    pub summary: String,
    /// Overall assessment: one of approve, request-changes, or comment.
    pub verdict: String,
    /// Key strengths of the PR.
    #[serde(default)]
    pub strengths: Vec<String>,
    /// Areas of concern or improvement.
    #[serde(default)]
    pub concerns: Vec<String>,
    /// Specific line-level comments.
    #[serde(default)]
    pub comments: Vec<PrReviewComment>,
    /// Suggested improvements (not blocking).
    #[serde(default)]
    pub suggestions: Vec<String>,
    /// Optional disclaimer about limitations (e.g., platform version validation).
    #[serde(default)]
    pub disclaimer: Option<String>,
}

/// Review event type for posting to GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEvent {
    /// Post as a comment without approval/request.
    Comment,
    /// Approve the PR.
    Approve,
    /// Request changes to the PR.
    RequestChanges,
}

impl std::fmt::Display for ReviewEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewEvent::Comment => write!(f, "COMMENT"),
            ReviewEvent::Approve => write!(f, "APPROVE"),
            ReviewEvent::RequestChanges => write!(f, "REQUEST_CHANGES"),
        }
    }
}

/// Structured PR label response from AI.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PrLabelResponse {
    /// Suggested labels for the PR.
    pub suggested_labels: Vec<String>,
}

/// Summary of a PR for release notes context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSummary {
    /// PR number.
    pub number: u64,
    /// PR title.
    pub title: String,
    /// PR description/body.
    pub body: String,
    /// Author login.
    pub author: String,
    /// Merged at timestamp.
    pub merged_at: Option<String>,
}

/// Structured release notes response from AI.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReleaseNotesResponse {
    /// Release theme/title.
    pub theme: String,
    /// 1-2 sentence narrative.
    pub narrative: String,
    /// Highlighted features.
    pub highlights: Vec<String>,
    /// Features section.
    pub features: Vec<String>,
    /// Fixes section.
    pub fixes: Vec<String>,
    /// Improvements section.
    pub improvements: Vec<String>,
    /// Documentation section.
    pub documentation: Vec<String>,
    /// Maintenance section.
    pub maintenance: Vec<String>,
    /// Contributor list.
    pub contributors: Vec<String>,
}
