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
