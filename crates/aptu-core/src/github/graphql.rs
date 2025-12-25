// SPDX-License-Identifier: Apache-2.0

//! GraphQL queries for GitHub API.
//!
//! Uses a single GraphQL query to fetch issues from multiple repositories
//! efficiently, avoiding multiple REST API calls.

use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, instrument};

use crate::ai::types::{IssueComment, RepoLabel, RepoMilestone};

/// Viewer permission level on a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ViewerPermission {
    /// Admin permission.
    Admin,
    /// Maintain permission.
    Maintain,
    /// Write permission.
    Write,
    /// Triage permission.
    Triage,
    /// Read permission.
    Read,
}

/// A GitHub issue from the GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueNode {
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Creation timestamp (ISO 8601).
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// Issue labels.
    pub labels: Labels,
    /// Issue URL (used by triage command).
    #[allow(dead_code)]
    pub url: String,
}

/// Labels container from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Labels {
    /// List of label nodes.
    pub nodes: Vec<LabelNode>,
}

/// A single label.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LabelNode {
    /// Label name.
    pub name: String,
}

/// Issues response for a single repository.
#[derive(Debug, Deserialize)]
pub struct RepoIssues {
    /// Repository name with owner (e.g., "block/goose").
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
    /// Issues container.
    pub issues: IssuesConnection,
}

/// Issues connection from GraphQL.
#[derive(Debug, Deserialize)]
pub struct IssuesConnection {
    /// List of issue nodes.
    pub nodes: Vec<IssueNode>,
}

/// Builds a GraphQL query to fetch issues from multiple repositories.
///
/// Uses GraphQL aliases to query all repos in a single request.
fn build_issues_query<R: AsRef<str>>(repos: &[(R, R)]) -> Value {
    let fragments: Vec<String> = repos
        .iter()
        .enumerate()
        .map(|(i, (owner, name))| {
            format!(
                r#"repo{i}: repository(owner: "{owner}", name: "{name}") {{
                    nameWithOwner
                    issues(
                        first: 10
                        states: OPEN
                        labels: ["good first issue"]
                        filterBy: {{ assignee: null }}
                        orderBy: {{ field: CREATED_AT, direction: DESC }}
                    ) {{
                        nodes {{
                            number
                            title
                            createdAt
                            labels(first: 5) {{ nodes {{ name }} }}
                            url
                        }}
                    }}
                }}"#,
                i = i,
                owner = owner.as_ref(),
                name = name.as_ref()
            )
        })
        .collect();

    let query = format!("query {{ {} }}", fragments.join("\n"));
    debug!(query_length = query.len(), "Built GraphQL query");
    json!({ "query": query })
}

/// Fetches open "good first issue" issues from multiple repositories.
///
/// Accepts a slice of (owner, name) tuples.
/// Returns a vector of (`repo_name`, issues) tuples.
#[instrument(skip(client, repos), fields(repo_count = repos.len()))]
pub async fn fetch_issues<R: AsRef<str>>(
    client: &Octocrab,
    repos: &[(R, R)],
) -> Result<Vec<(String, Vec<IssueNode>)>> {
    if repos.is_empty() {
        return Ok(vec![]);
    }

    let query = build_issues_query(repos);
    debug!("Executing GraphQL query");

    // Execute the GraphQL query
    let response: Value = client
        .graphql(&query)
        .await
        .context("Failed to execute GraphQL query")?;

    // Check for GraphQL errors
    if let Some(errors) = response.get("errors") {
        let error_msg = serde_json::to_string_pretty(errors).unwrap_or_default();
        anyhow::bail!("GraphQL error: {error_msg}");
    }

    // Parse the response
    let data = response
        .get("data")
        .context("Missing 'data' field in GraphQL response")?;

    let mut results = Vec::with_capacity(repos.len());

    for i in 0..repos.len() {
        let key = format!("repo{i}");
        if let Some(repo_data) = data.get(&key) {
            // Repository might not exist or be private
            if repo_data.is_null() {
                debug!(repo = key, "Repository not found or inaccessible");
                continue;
            }

            let repo_issues: RepoIssues = serde_json::from_value(repo_data.clone())
                .with_context(|| format!("Failed to parse repository data for {key}"))?;

            let issue_count = repo_issues.issues.nodes.len();
            if issue_count > 0 {
                debug!(
                    repo = %repo_issues.name_with_owner,
                    issues = issue_count,
                    "Found issues"
                );
                results.push((repo_issues.name_with_owner, repo_issues.issues.nodes));
            }
        }
    }

    debug!(
        total_repos = results.len(),
        "Fetched issues from repositories"
    );
    Ok(results)
}

/// Repository label from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoLabelNode {
    /// Label name.
    pub name: String,
    /// Label description.
    pub description: Option<String>,
    /// Label color (hex code without #).
    pub color: String,
}

impl From<RepoLabelNode> for RepoLabel {
    fn from(node: RepoLabelNode) -> Self {
        RepoLabel {
            name: node.name,
            description: node.description.unwrap_or_default(),
            color: node.color,
        }
    }
}

/// Repository labels connection from GraphQL.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoLabelsConnection {
    /// List of label nodes.
    pub nodes: Vec<RepoLabelNode>,
}

/// Repository milestone from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoMilestoneNode {
    /// Milestone number.
    pub number: u64,
    /// Milestone title.
    pub title: String,
    /// Milestone description.
    pub description: Option<String>,
}

impl From<RepoMilestoneNode> for RepoMilestone {
    fn from(node: RepoMilestoneNode) -> Self {
        RepoMilestone {
            number: node.number,
            title: node.title,
            description: node.description.unwrap_or_default(),
        }
    }
}

/// Repository milestones connection from GraphQL.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoMilestonesConnection {
    /// List of milestone nodes.
    pub nodes: Vec<RepoMilestoneNode>,
}

/// Issue comment from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueCommentNode {
    /// Comment author login.
    pub author: Author,
    /// Comment body.
    pub body: String,
}

impl From<IssueCommentNode> for IssueComment {
    fn from(node: IssueCommentNode) -> Self {
        IssueComment {
            author: node.author.login,
            body: node.body,
        }
    }
}

/// Author information from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Author {
    /// Author login.
    pub login: String,
}

/// Comments connection from GraphQL.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommentsConnection {
    /// List of comment nodes.
    pub nodes: Vec<IssueCommentNode>,
}

/// Issue from GraphQL response for triage.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueNodeDetailed {
    /// Issue number.
    pub number: u64,
    /// Issue title.
    pub title: String,
    /// Issue body.
    pub body: Option<String>,
    /// Issue URL.
    pub url: String,
    /// Issue labels.
    pub labels: Labels,
    /// Issue comments.
    pub comments: CommentsConnection,
}

/// Repository data from GraphQL response for triage.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepositoryData {
    /// Repository name with owner.
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
    /// Repository labels.
    pub labels: RepoLabelsConnection,
    /// Repository milestones.
    pub milestones: RepoMilestonesConnection,
    /// Repository primary language.
    #[serde(rename = "primaryLanguage")]
    pub primary_language: Option<LanguageNode>,
    /// Viewer permission level on the repository.
    #[serde(rename = "viewerPermission")]
    pub viewer_permission: Option<ViewerPermission>,
}

/// Language information from GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanguageNode {
    /// Language name.
    pub name: String,
}

/// Full response for issue with repo context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueWithRepoContextResponse {
    /// The issue.
    pub issue: IssueNodeDetailed,
    /// The repository.
    pub repository: RepositoryData,
}

/// Builds a GraphQL query to fetch an issue with repository context.
fn build_issue_with_repo_context_query(owner: &str, repo: &str, number: u64) -> Value {
    let query = format!(
        r#"query {{
            issue: repository(owner: "{owner}", name: "{repo}") {{
                issue(number: {number}) {{
                    number
                    title
                    body
                    url
                    labels(first: 10) {{
                        nodes {{
                            name
                        }}
                    }}
                    comments(first: 5) {{
                        nodes {{
                            author {{
                                login
                            }}
                            body
                        }}
                    }}
                }}
            }}
            repository(owner: "{owner}", name: "{repo}") {{
                nameWithOwner
                viewerPermission
                labels(first: 100) {{
                    nodes {{
                        name
                        description
                        color
                    }}
                }}
                milestones(first: 50, states: OPEN) {{
                    nodes {{
                        number
                        title
                        description
                    }}
                }}
                primaryLanguage {{
                    name
                }}
            }}
        }}"#
    );

    json!({ "query": query })
}

/// Fetches an issue with repository context (labels, milestones) in a single GraphQL call.
///
/// # Errors
///
/// Returns an error if the GraphQL query fails or the issue is not found.
#[instrument(skip(client), fields(owner = %owner, repo = %repo, number = number))]
pub async fn fetch_issue_with_repo_context(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<(IssueNodeDetailed, RepositoryData)> {
    debug!("Fetching issue with repository context");

    let query = build_issue_with_repo_context_query(owner, repo, number);
    debug!("Executing GraphQL query for issue with repo context");

    let response: Value = client
        .graphql(&query)
        .await
        .context("Failed to execute GraphQL query")?;

    // Check for GraphQL errors
    if let Some(errors) = response.get("errors") {
        let error_msg = serde_json::to_string_pretty(errors).unwrap_or_default();
        anyhow::bail!("GraphQL error: {error_msg}");
    }

    let data = response
        .get("data")
        .context("Missing 'data' field in GraphQL response")?;

    // Extract issue from nested structure
    let issue_data = data
        .get("issue")
        .and_then(|v| v.get("issue"))
        .context("Issue not found in GraphQL response")?;

    let issue: IssueNodeDetailed =
        serde_json::from_value(issue_data.clone()).context("Failed to parse issue data")?;

    let repo_data = data
        .get("repository")
        .context("Repository not found in GraphQL response")?;

    let repository: RepositoryData =
        serde_json::from_value(repo_data.clone()).context("Failed to parse repository data")?;

    debug!(
        issue_number = issue.number,
        labels_count = repository.labels.nodes.len(),
        milestones_count = repository.milestones.nodes.len(),
        "Fetched issue with repository context"
    );

    Ok((issue, repository))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_single_repo() {
        let repos = [("block", "goose")];

        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert!(query_str.contains("repo0: repository(owner: \"block\", name: \"goose\")"));
        assert!(query_str.contains("labels: [\"good first issue\"]"));
        assert!(query_str.contains("states: OPEN"));
    }

    #[test]
    fn build_query_multiple_repos() {
        let repos = [("block", "goose"), ("astral-sh", "ruff")];

        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert!(query_str.contains("repo0: repository(owner: \"block\", name: \"goose\")"));
        assert!(query_str.contains("repo1: repository(owner: \"astral-sh\", name: \"ruff\")"));
    }

    #[test]
    fn build_query_empty_repos() {
        let repos: [(&str, &str); 0] = [];
        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert_eq!(query_str, "query {  }");
    }
}
