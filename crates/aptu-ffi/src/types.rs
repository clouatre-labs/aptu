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

#[derive(Clone, Debug, uniffi::Record, Serialize, Deserialize)]
pub struct FfiTokenStatus {
    pub is_authenticated: bool,
    pub token_source: String,
    pub expires_at: Option<String>,
}
