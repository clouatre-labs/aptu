// SPDX-License-Identifier: Apache-2.0

//! GraphQL queries for GitHub API.
//!
//! Uses a single GraphQL query to fetch issues from multiple repositories
//! efficiently, avoiding multiple REST API calls.

use anyhow::{Context, Result};
#[cfg(not(target_arch = "wasm32"))]
use backon::Retryable;
#[cfg(not(target_arch = "wasm32"))]
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, instrument};

use crate::ai::types::{IssueComment, RepoLabel, RepoMilestone};
use crate::error::{AptuError, ResourceType};
#[cfg(not(target_arch = "wasm32"))]
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

impl std::fmt::Display for ViewerPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Admin => "Admin",
            Self::Maintain => "Maintain",
            Self::Write => "Write",
            Self::Triage => "Triage",
            Self::Read => "Read",
        };
        write!(f, "{name}")
    }
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
#[cfg(not(target_arch = "wasm32"))]
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

    let mut results = Vec::with_capacity(repos.len());

    for i in 0..repos.len() {
        let key = format!("repo{i}");
        if let Some(repo_data) = response.get(&key) {
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
    /// Comment ID (GraphQL ID scalar, an opaque string).
    pub id: String,
    /// Comment author login. GitHub returns `null` for deleted users, which
    /// deserializes to `None` and maps to the "ghost" placeholder.
    pub author: Option<Author>,
    /// Comment body.
    pub body: String,
}

/// Placeholder login used when a comment author has been deleted.
const GHOST_LOGIN: &str = "ghost";

impl From<IssueCommentNode> for IssueComment {
    fn from(node: IssueCommentNode) -> Self {
        IssueComment {
            id: node.id,
            author: node
                .author
                .map_or_else(|| GHOST_LOGIN.to_owned(), |a| a.login),
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

/// Builds the GraphQL query fetching the viewer permission for a repository.
#[cfg(not(target_arch = "wasm32"))]
fn build_viewer_permission_query(owner: &str, repo: &str) -> Value {
    let query = format!(
        r#"query {{ repository(owner: "{owner}", name: "{repo}") {{ viewerPermission }} }}"#
    );
    json!({ "query": query })
}

/// Fetches the viewer permission level for a repository via GraphQL.
///
/// Returns `None` when the field is null or holds an unrecognized value; callers
/// treat unknown permissions as allowed.
///
/// # Errors
///
/// Returns an error if the GraphQL query fails.
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo))]
pub async fn fetch_repo_viewer_permission(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Option<ViewerPermission>> {
    let query = build_viewer_permission_query(owner, repo);
    debug!("Executing GraphQL query for viewer permission");

    let response: Value = match client.graphql(&query).await {
        Ok(value) => value,
        Err(e) => {
            debug!(error = %e, "Viewer permission query failed; treating as unknown");
            return Ok(None);
        }
    };

    // Octocrab's graphql() returns the unwrapped `data` object directly
    // (same shape as fetch_issue_with_repo_context / fetch_issues), so the
    // repository payload sits at the top level of the response.
    let perm = parse_viewer_permission(&response);
    debug!(viewer_permission = ?perm, "Fetched viewer permission");
    Ok(perm)
}

/// Parses the viewer permission from an unwrapped GraphQL `data` object.
///
/// Matches the shape used by the rest of this module: the response is the
/// `data` map itself, so the field lives at `repository.viewerPermission`.
fn parse_viewer_permission(response: &Value) -> Option<ViewerPermission> {
    response["repository"]["viewerPermission"]
        .as_str()
        .and_then(|s| serde_json::from_value::<ViewerPermission>(Value::String(s.to_string())).ok())
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
                            id
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
                || err
                    .get("extensions")
                    .and_then(|extensions| extensions.get("type"))
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t == "NOT_FOUND")
                || err
                    .get("message")
                    .and_then(|message| message.as_str())
                    .is_some_and(|message| message.to_ascii_lowercase().contains("not found"))
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
#[cfg(not(target_arch = "wasm32"))]
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

    let response: Value = match client.graphql(&query).await {
        Ok(response) => response,
        Err(error) => {
            if let octocrab::Error::Graphql { source, .. } = &error
                && is_not_found_error(&serde_json::json!(source.0))
            {
                debug!("GraphQL NOT_FOUND error, checking if reference is a PR");
                if (client.pulls(owner, repo).get(number).await).is_ok() {
                    return Err(AptuError::TypeMismatch {
                        number,
                        expected: ResourceType::Issue,
                        actual: ResourceType::PullRequest,
                    }
                    .into());
                }
            }
            return Err(anyhow::anyhow!(error).context("Failed to execute GraphQL query"));
        }
    };

    // Extract issue from nested structure
    let issue_data = response.get("issue").and_then(|v| v.get("issue"));

    let Some(issue_val) = issue_data.filter(|v| !v.is_null()) else {
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
    };

    let issue: IssueNodeDetailed =
        serde_json::from_value(issue_val.clone()).context("Failed to parse issue data")?;

    let repo_data = response
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
    fn not_found_error_detected() {
        assert!(is_not_found_error(
            &serde_json::json!([{"type": "NOT_FOUND"}])
        ));
    }

    #[test]
    fn unrelated_error_not_detected() {
        assert!(!is_not_found_error(
            &serde_json::json!([{"type": "FORBIDDEN"}])
        ));
    }

    #[test]
    fn malformed_errors_not_detected() {
        assert!(!is_not_found_error(
            &serde_json::json!({"type": "NOT_FOUND"})
        ));
    }

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
    fn parse_viewer_permission_reads_unwrapped_data_shape() {
        // Matches the shape octocrab's graphql() returns elsewhere in this
        // module: the `data` object itself, not the full GraphQL envelope.
        let response = serde_json::json!({
            "repository": { "viewerPermission": "READ" }
        });
        let perm = parse_viewer_permission(&response);
        assert_eq!(perm, Some(ViewerPermission::Read));
    }

    #[test]
    fn parse_viewer_permission_handles_null_and_unknown() {
        let null_response = serde_json::json!({ "repository": { "viewerPermission": null } });
        assert_eq!(parse_viewer_permission(&null_response), None);
        let unknown = serde_json::json!({ "repository": { "viewerPermission": "SOMETHING_NEW" } });
        assert_eq!(parse_viewer_permission(&unknown), None);
        let missing = serde_json::json!({});
        assert_eq!(parse_viewer_permission(&missing), None);
    }

    #[test]
    fn build_viewer_permission_query_includes_field() {
        let query = build_viewer_permission_query("block", "goose");
        let query_str = query["query"].as_str().unwrap();
        assert!(query_str.contains("repository(owner: \"block\", name: \"goose\")"));
        assert!(query_str.contains("viewerPermission"));
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
#[cfg(not(target_arch = "wasm32"))]
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
        .get("repository")
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
