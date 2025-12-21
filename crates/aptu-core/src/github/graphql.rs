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

use crate::repos::CuratedRepo;

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
fn build_issues_query(repos: &[CuratedRepo]) -> Value {
    let fragments: Vec<String> = repos
        .iter()
        .enumerate()
        .map(|(i, repo)| {
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
                owner = repo.owner,
                name = repo.name
            )
        })
        .collect();

    let query = format!("query {{ {} }}", fragments.join("\n"));
    debug!(query_length = query.len(), "Built GraphQL query");
    json!({ "query": query })
}

/// Fetches open "good first issue" issues from all curated repositories.
///
/// Returns a vector of (`repo_name`, issues) tuples.
#[instrument(skip(client, repos), fields(repo_count = repos.len()))]
pub async fn fetch_issues(
    client: &Octocrab,
    repos: &[CuratedRepo],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_single_repo() {
        let repos = [CuratedRepo {
            owner: "block",
            name: "goose",
            language: "Rust",
            description: "AI agent",
        }];

        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert!(query_str.contains("repo0: repository(owner: \"block\", name: \"goose\")"));
        assert!(query_str.contains("labels: [\"good first issue\"]"));
        assert!(query_str.contains("states: OPEN"));
    }

    #[test]
    fn build_query_multiple_repos() {
        let repos = [
            CuratedRepo {
                owner: "block",
                name: "goose",
                language: "Rust",
                description: "AI agent",
            },
            CuratedRepo {
                owner: "astral-sh",
                name: "ruff",
                language: "Rust",
                description: "Linter",
            },
        ];

        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert!(query_str.contains("repo0: repository(owner: \"block\", name: \"goose\")"));
        assert!(query_str.contains("repo1: repository(owner: \"astral-sh\", name: \"ruff\")"));
    }

    #[test]
    fn build_query_empty_repos() {
        let repos: [CuratedRepo; 0] = [];
        let query = build_issues_query(&repos);
        let query_str = query["query"].as_str().unwrap();

        assert_eq!(query_str, "query {  }");
    }
}
