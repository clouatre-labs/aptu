// SPDX-License-Identifier: Apache-2.0

//! GraphQL queries for GitHub API.
//!
//! Uses a single GraphQL query to fetch issues from multiple repositories
//! efficiently, avoiding multiple REST API calls.

use anyhow::{Context, Result};
use backon::Retryable;
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, instrument};

use crate::ai::types::{IssueComment, RepoLabel, RepoMilestone};
use crate::error::{AptuError, ResourceType};
use crate::retry::retry_backoff;

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

    // Execute the GraphQL query with retry logic
    let response: Value =
        (|| async { client.graphql(&query).await.map_err(|e| anyhow::anyhow!(e)) })
            .retry(retry_backoff())
            .notify(|err, dur| {
                tracing::warn!(
                    error = %err,
                    retry_after = ?dur,
                    "Retrying fetch_issues (GraphQL query)"
                );
            })
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
    /// Total count of comments.
    #[serde(rename = "totalCount")]
    pub total_count: u32,
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
    /// Issue author.
    pub author: Option<Author>,
    /// Issue creation timestamp (ISO 8601).
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// Issue last update timestamp (ISO 8601).
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
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
                    author {{
                        login
                    }}
                    createdAt
                    updatedAt
                    labels(first: 10) {{
                        nodes {{
                            name
                        }}
                    }}
                    comments(first: 5) {{
                        totalCount
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

/// Checks if any error in the GraphQL errors array has type=`NOT_FOUND`.
fn is_not_found_error(errors: &Value) -> bool {
    if let Some(arr) = errors.as_array() {
        arr.iter().any(|err| {
            err.get("type")
                .and_then(|t| t.as_str())
                .is_some_and(|t| t == "NOT_FOUND")
        })
    } else {
        false
    }
}

/// Fetches an issue with repository context (labels, milestones) in a single GraphQL call.
///
/// # Errors
///
/// Returns an error if the GraphQL query fails or the issue is not found.
/// If the issue is not found but a PR with the same number exists, returns a `TypeMismatch` error.
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

        // Only attempt fallback for NOT_FOUND errors to avoid unnecessary API calls
        if is_not_found_error(errors) {
            debug!("GraphQL NOT_FOUND error, checking if reference is a PR");

            // Try to fetch as a PR to provide a better error message
            if (client.pulls(owner, repo).get(number).await).is_ok() {
                return Err(AptuError::TypeMismatch {
                    number,
                    expected: ResourceType::Issue,
                    actual: ResourceType::PullRequest,
                }
                .into());
            }
        }

        // Not a PR or not a NOT_FOUND error, return the original GraphQL error
        anyhow::bail!("GraphQL error: {error_msg}");
    }

    let data = response
        .get("data")
        .context("Missing 'data' field in GraphQL response")?;

    // Extract issue from nested structure
    let issue_data = data.get("issue").and_then(|v| v.get("issue"));

    // Check if issue is null (not found)
    if issue_data.is_none() || issue_data.is_some_and(serde_json::Value::is_null) {
        debug!("Issue not found in GraphQL response, checking if reference is a PR");

        // Try to fetch as a PR to provide a better error message
        if (client.pulls(owner, repo).get(number).await).is_ok() {
            return Err(AptuError::TypeMismatch {
                number,
                expected: ResourceType::Issue,
                actual: ResourceType::PullRequest,
            }
            .into());
        }

        // Not a PR, return the original error
        anyhow::bail!("Issue not found in GraphQL response");
    }

    let issue: IssueNodeDetailed = serde_json::from_value(issue_data.unwrap().clone())
        .context("Failed to parse issue data")?;

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

/// Target of a reference (either a Tag or Commit).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RefTarget {
    /// A tag object.
    Tag(TagTarget),
    /// A commit object.
    Commit(CommitTarget),
}

/// A tag object from the GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TagTarget {
    /// The commit that this tag points to.
    pub target: CommitTarget,
}

/// A commit object from the GraphQL response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitTarget {
    /// The commit SHA.
    pub oid: String,
}

/// Build a GraphQL query to resolve a tag to its commit SHA.
///
/// Uses inline fragments to handle both Tag and Commit target types.
fn build_tag_resolution_query(owner: &str, repo: &str, ref_name: &str) -> Value {
    let query = format!(
        r#"query {{
  repository(owner: "{owner}", name: "{repo}") {{
    ref(qualifiedName: "refs/tags/{ref_name}") {{
      target {{
        ... on Tag {{
          target {{
            oid
          }}
        }}
        ... on Commit {{
          oid
        }}
      }}
    }}
  }}
}}"#
    );

    json!({
        "query": query,
    })
}

/// Resolve a tag to its commit SHA using GraphQL.
///
/// Handles both lightweight tags (which point directly to commits) and
/// annotated tags (which have a Tag object that points to a commit).
///
/// # Arguments
///
/// * `client` - Octocrab GitHub client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `tag_name` - Tag name to resolve
///
/// # Returns
///
/// The commit SHA for the tag, or None if the tag doesn't exist.
#[instrument(skip(client))]
pub async fn resolve_tag_to_commit_sha(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    tag_name: &str,
) -> Result<Option<String>> {
    let query = build_tag_resolution_query(owner, repo, tag_name);

    let response = (|| async {
        client
            .graphql::<serde_json::Value>(&query)
            .await
            .context("GraphQL query failed")
    })
    .retry(&retry_backoff())
    .await?;

    debug!("GraphQL response: {:?}", response);

    // Extract the target from the response
    let target = response
        .get("data")
        .and_then(|data| data.get("repository"))
        .and_then(|repo| repo.get("ref"))
        .and_then(|ref_obj| ref_obj.get("target"));

    match target {
        Some(target_value) => {
            // Try to deserialize as RefTarget to handle both Tag and Commit cases
            match serde_json::from_value::<RefTarget>(target_value.clone()) {
                Ok(RefTarget::Tag(tag)) => Ok(Some(tag.target.oid)),
                Ok(RefTarget::Commit(commit)) => Ok(Some(commit.oid)),
                Err(_) => Ok(None),
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tag_resolution_tests {
    use super::*;

    #[test]
    fn build_tag_resolution_query_correct_syntax() {
        let query = build_tag_resolution_query("owner", "repo", "v1.0.0");
        let query_str = query["query"].as_str().unwrap();

        assert!(query_str.contains("repository(owner: \"owner\", name: \"repo\")"));
        assert!(query_str.contains("ref(qualifiedName: \"refs/tags/v1.0.0\")"));
        assert!(query_str.contains("... on Tag"));
        assert!(query_str.contains("... on Commit"));
        assert!(query_str.contains("oid"));
    }
}
