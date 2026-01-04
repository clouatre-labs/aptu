// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiCuratedRepo {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub open_issues_count: u32,
    pub last_activity: String,
}

impl From<&aptu_core::repos::CuratedRepo> for FfiCuratedRepo {
    fn from(repo: &aptu_core::repos::CuratedRepo) -> Self {
        FfiCuratedRepo {
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            description: repo.description.clone(),
            language: repo.language.clone(),
            open_issues_count: 0, // TODO: fetch from GitHub API in Phase 2
            last_activity: "unknown".to_string(), // TODO: fetch from GitHub API in Phase 2
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiIssueNode {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

impl From<aptu_core::github::graphql::IssueNode> for FfiIssueNode {
    fn from(issue: aptu_core::github::graphql::IssueNode) -> Self {
        let labels = issue
            .labels
            .nodes
            .iter()
            .map(|label| label.name.clone())
            .collect();

        FfiIssueNode {
            number: issue.number,
            title: issue.title,
            body: String::new(),
            labels,
            created_at: issue.created_at,
            updated_at: String::new(),
            url: issue.url,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiIssueDetails {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub author: String,
    pub comments_count: u32,
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiRelatedIssue {
    pub number: u64,
    pub title: String,
    pub reason: String,
}

impl From<aptu_core::ai::types::RelatedIssue> for FfiRelatedIssue {
    fn from(issue: aptu_core::ai::types::RelatedIssue) -> Self {
        FfiRelatedIssue {
            number: issue.number,
            title: issue.title,
            reason: issue.reason,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiContributorGuidance {
    pub beginner_friendly: bool,
    pub reasoning: String,
}

impl From<aptu_core::ai::types::ContributorGuidance> for FfiContributorGuidance {
    fn from(guidance: aptu_core::ai::types::ContributorGuidance) -> Self {
        FfiContributorGuidance {
            beginner_friendly: guidance.beginner_friendly,
            reasoning: guidance.reasoning,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiTriageResponse {
    pub summary: String,
    pub suggested_labels: Vec<String>,
    pub clarifying_questions: Vec<String>,
    pub potential_duplicates: Vec<String>,
    pub related_issues: Vec<FfiRelatedIssue>,
    pub status_note: Option<String>,
    pub contributor_guidance: Option<FfiContributorGuidance>,
    pub implementation_approach: Option<String>,
    pub suggested_milestone: Option<String>,
}

impl From<aptu_core::ai::types::TriageResponse> for FfiTriageResponse {
    fn from(triage: aptu_core::ai::types::TriageResponse) -> Self {
        FfiTriageResponse {
            summary: triage.summary,
            suggested_labels: triage.suggested_labels,
            clarifying_questions: triage.clarifying_questions,
            potential_duplicates: triage.potential_duplicates,
            related_issues: triage
                .related_issues
                .into_iter()
                .map(FfiRelatedIssue::from)
                .collect(),
            status_note: triage.status_note,
            contributor_guidance: triage
                .contributor_guidance
                .map(FfiContributorGuidance::from),
            implementation_approach: triage.implementation_approach,
            suggested_milestone: triage.suggested_milestone,
        }
    }
}

impl From<FfiTriageResponse> for aptu_core::ai::types::TriageResponse {
    fn from(ffi_triage: FfiTriageResponse) -> Self {
        aptu_core::ai::types::TriageResponse {
            summary: ffi_triage.summary,
            suggested_labels: ffi_triage.suggested_labels,
            clarifying_questions: ffi_triage.clarifying_questions,
            potential_duplicates: ffi_triage.potential_duplicates,
            related_issues: ffi_triage
                .related_issues
                .into_iter()
                .map(|issue| aptu_core::ai::types::RelatedIssue {
                    number: issue.number,
                    title: issue.title,
                    reason: issue.reason,
                })
                .collect(),
            status_note: ffi_triage.status_note,
            contributor_guidance: ffi_triage.contributor_guidance.map(|guidance| {
                aptu_core::ai::types::ContributorGuidance {
                    beginner_friendly: guidance.beginner_friendly,
                    reasoning: guidance.reasoning,
                }
            }),
            implementation_approach: ffi_triage.implementation_approach,
            suggested_milestone: ffi_triage.suggested_milestone,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiTokenStatus {
    pub is_authenticated: bool,
    pub token_source: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Copy, Debug, uniffi::Enum, Serialize, Deserialize)]
pub enum FfiModelProvider {
    OpenRouter,
    Ollama,
    Mlx,
}

impl From<aptu_core::ai::models::ModelProvider> for FfiModelProvider {
    fn from(provider: aptu_core::ai::models::ModelProvider) -> Self {
        match provider {
            aptu_core::ai::models::ModelProvider::OpenRouter => FfiModelProvider::OpenRouter,
            aptu_core::ai::models::ModelProvider::Ollama => FfiModelProvider::Ollama,
            aptu_core::ai::models::ModelProvider::Mlx => FfiModelProvider::Mlx,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiAiModel {
    pub display_name: String,
    pub identifier: String,
    pub provider: FfiModelProvider,
    pub is_free: bool,
    pub context_window: u32,
}

impl From<aptu_core::ai::models::AiModel> for FfiAiModel {
    fn from(model: aptu_core::ai::models::AiModel) -> Self {
        FfiAiModel {
            display_name: model.display_name,
            identifier: model.identifier,
            provider: FfiModelProvider::from(model.provider),
            is_free: model.is_free,
            context_window: model.context_window,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiApplyResult {
    pub applied_labels: Vec<String>,
    pub applied_milestone: Option<String>,
    pub warnings: Vec<String>,
}

impl From<aptu_core::ApplyResult> for FfiApplyResult {
    fn from(result: aptu_core::ApplyResult) -> Self {
        FfiApplyResult {
            applied_labels: result.applied_labels,
            applied_milestone: result.applied_milestone,
            warnings: result.warnings,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiReleaseNotesResponse {
    pub theme: String,
    pub narrative: String,
    pub highlights: Vec<String>,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub improvements: Vec<String>,
    pub documentation: Vec<String>,
    pub maintenance: Vec<String>,
    pub contributors: Vec<String>,
}

impl From<aptu_core::ai::types::ReleaseNotesResponse> for FfiReleaseNotesResponse {
    fn from(response: aptu_core::ai::types::ReleaseNotesResponse) -> Self {
        FfiReleaseNotesResponse {
            theme: response.theme,
            narrative: response.narrative,
            highlights: response.highlights,
            features: response.features,
            fixes: response.fixes,
            improvements: response.improvements,
            documentation: response.documentation,
            maintenance: response.maintenance,
            contributors: response.contributors,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiCreateIssueResponse {
    pub formatted_title: String,
    pub formatted_body: String,
    pub suggested_labels: Vec<String>,
}

impl From<aptu_core::ai::types::CreateIssueResponse> for FfiCreateIssueResponse {
    fn from(response: aptu_core::ai::types::CreateIssueResponse) -> Self {
        FfiCreateIssueResponse {
            formatted_title: response.formatted_title,
            formatted_body: response.formatted_body,
            suggested_labels: response.suggested_labels,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiPostIssueResult {
    pub issue_url: String,
    pub issue_number: u64,
}

impl From<(String, u64)> for FfiPostIssueResult {
    fn from((url, number): (String, u64)) -> Self {
        FfiPostIssueResult {
            issue_url: url,
            issue_number: number,
        }
    }
}

/// Represents a discovered repository from GitHub search.
#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiDiscoveredRepo {
    /// Repository owner/organization name
    pub owner: String,
    /// Repository name
    pub name: String,
    /// Primary programming language (if available)
    pub language: Option<String>,
    /// Repository description
    pub description: Option<String>,
    /// Number of GitHub stars
    pub stars: u32,
}

impl From<aptu_core::repos::discovery::DiscoveredRepo> for FfiDiscoveredRepo {
    fn from(repo: aptu_core::repos::discovery::DiscoveredRepo) -> Self {
        FfiDiscoveredRepo {
            owner: repo.owner,
            name: repo.name,
            language: repo.language,
            description: repo.description,
            stars: repo.stars,
        }
    }
}

/// Result of auto-labeling a pull request.
#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiLabelPrResult {
    /// Pull request number
    pub pr_number: u64,
    /// Pull request title
    pub title: String,
    /// Pull request body/description
    pub body: String,
    /// Labels that were applied to the PR
    pub applied_labels: Vec<String>,
}

impl From<(u64, String, String, Vec<String>)> for FfiLabelPrResult {
    fn from((pr_number, title, body, applied_labels): (u64, String, String, Vec<String>)) -> Self {
        FfiLabelPrResult {
            pr_number,
            title,
            body,
            applied_labels,
        }
    }
}

/// FFI-compatible severity level for PR review comments.
#[derive(Clone, Copy, Debug, uniffi::Enum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FfiCommentSeverity {
    Info,
    Suggestion,
    Warning,
    Issue,
}

impl From<aptu_core::ai::types::CommentSeverity> for FfiCommentSeverity {
    fn from(severity: aptu_core::ai::types::CommentSeverity) -> Self {
        match severity {
            aptu_core::ai::types::CommentSeverity::Info => FfiCommentSeverity::Info,
            aptu_core::ai::types::CommentSeverity::Suggestion => FfiCommentSeverity::Suggestion,
            aptu_core::ai::types::CommentSeverity::Warning => FfiCommentSeverity::Warning,
            aptu_core::ai::types::CommentSeverity::Issue => FfiCommentSeverity::Issue,
        }
    }
}

/// FFI wrapper for a single PR review comment.
#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiPrReviewComment {
    /// File path the comment applies to.
    pub file: String,
    /// Line number in the diff (optional for general file comments).
    pub line: Option<u32>,
    /// The comment text.
    pub comment: String,
    /// Severity level for the comment.
    pub severity: FfiCommentSeverity,
}

impl From<aptu_core::ai::types::PrReviewComment> for FfiPrReviewComment {
    fn from(comment: aptu_core::ai::types::PrReviewComment) -> Self {
        FfiPrReviewComment {
            file: comment.file,
            line: comment.line,
            comment: comment.comment,
            severity: comment.severity.into(),
        }
    }
}

/// FFI wrapper for AI usage statistics.
#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiAiStats {
    /// Model used for analysis.
    pub model: String,
    /// Number of input tokens.
    pub input_tokens: u64,
    /// Number of output tokens.
    pub output_tokens: u64,
    /// Duration of the API call in milliseconds.
    pub duration_ms: u64,
    /// Cost in USD (from `OpenRouter` API, `None` if not reported).
    pub cost_usd: Option<f64>,
    /// Fallback provider used if primary failed (None if primary succeeded).
    pub fallback_provider: Option<String>,
}

impl From<aptu_core::history::AiStats> for FfiAiStats {
    fn from(stats: aptu_core::history::AiStats) -> Self {
        FfiAiStats {
            model: stats.model,
            input_tokens: stats.input_tokens,
            output_tokens: stats.output_tokens,
            duration_ms: stats.duration_ms,
            cost_usd: stats.cost_usd,
            fallback_provider: stats.fallback_provider,
        }
    }
}

/// FFI wrapper for PR review response.
#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiPrReviewResponse {
    /// Overall summary of the PR (2-3 sentences).
    pub summary: String,
    /// Overall assessment: one of approve, request-changes, or comment.
    pub verdict: String,
    /// Key strengths of the PR.
    pub strengths: Vec<String>,
    /// Areas of concern or improvement.
    pub concerns: Vec<String>,
    /// Specific line-level comments.
    pub comments: Vec<FfiPrReviewComment>,
    /// Suggested improvements (not blocking).
    pub suggestions: Vec<String>,
    /// Optional disclaimer about limitations (e.g., platform version validation).
    pub disclaimer: Option<String>,
}

impl From<aptu_core::ai::types::PrReviewResponse> for FfiPrReviewResponse {
    fn from(response: aptu_core::ai::types::PrReviewResponse) -> Self {
        FfiPrReviewResponse {
            summary: response.summary,
            verdict: response.verdict,
            strengths: response.strengths,
            concerns: response.concerns,
            comments: response
                .comments
                .into_iter()
                .map(FfiPrReviewComment::from)
                .collect(),
            suggestions: response.suggestions,
            disclaimer: response.disclaimer,
        }
    }
}
