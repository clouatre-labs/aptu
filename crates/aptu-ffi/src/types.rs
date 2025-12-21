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
            owner: repo.owner.to_string(),
            name: repo.name.to_string(),
            description: repo.description.to_string(),
            language: repo.language.to_string(),
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
pub struct FfiTriageResponse {
    pub summary: String,
    pub suggested_labels: Vec<String>,
    pub clarifying_questions: Vec<String>,
    pub potential_duplicates: Vec<String>,
}

impl From<aptu_core::ai::types::TriageResponse> for FfiTriageResponse {
    fn from(triage: aptu_core::ai::types::TriageResponse) -> Self {
        FfiTriageResponse {
            summary: triage.summary,
            suggested_labels: triage.suggested_labels,
            clarifying_questions: triage.clarifying_questions,
            potential_duplicates: triage.potential_duplicates,
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
