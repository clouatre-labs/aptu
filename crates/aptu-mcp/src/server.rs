// SPDX-License-Identifier: Apache-2.0

//! MCP server implementation combining tools, prompts, and resources.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{
        router::prompt::PromptRouter, router::tool::ToolRouter, wrapper::Parameters,
    },
    model::{
        AnnotateAble, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult,
        PaginatedRequestParams, PromptMessage, PromptMessageRole, RawResource, RawResourceTemplate,
        ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents,
        ResourceTemplate, ServerCapabilities, ServerInfo,
    },
    prompt, prompt_handler, prompt_router,
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::auth::EnvTokenProvider;
use crate::error::{aptu_error_to_mcp, generic_to_mcp_error};
use aptu_core::TokenProvider;

// ---------------------------------------------------------------------------
// Tool parameter structs
// ---------------------------------------------------------------------------

/// Parameters for triaging a GitHub issue.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Triage a GitHub issue using AI analysis")]
pub struct TriageIssueParams {
    /// Issue reference (e.g. "owner/repo#123" or full URL).
    #[schemars(description = "Issue reference such as owner/repo#123 or a GitHub URL")]
    pub issue_ref: String,
}

/// Parameters for reviewing a pull request.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Review a GitHub pull request using AI analysis")]
pub struct ReviewPrParams {
    /// PR reference (e.g. "owner/repo#456" or full URL).
    #[schemars(description = "PR reference such as owner/repo#456 or a GitHub URL")]
    pub pr_ref: String,
}

/// Parameters for scanning a diff for security issues.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Scan a code diff for security vulnerabilities")]
pub struct ScanSecurityParams {
    /// Unified diff text to scan.
    #[schemars(
        description = "Unified diff text to scan for security issues (output of git diff, git diff --staged, or similar). No stated size limit."
    )]
    pub diff: String,
}

/// Parameters for posting a triage comment.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Post an AI triage comment on a GitHub issue")]
pub struct PostTriageParams {
    /// Issue reference (e.g. "owner/repo#123" or full URL).
    #[schemars(description = "Issue reference such as owner/repo#123 or a GitHub URL")]
    pub issue_ref: String,
}

/// Review event type for posting PR reviews.
#[derive(Debug, Deserialize, JsonSchema, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEventParam {
    /// Approve the pull request.
    Approve,
    /// Request changes on the pull request.
    RequestChanges,
    /// Comment on the pull request without approval or changes.
    Comment,
}

impl From<ReviewEventParam> for aptu_core::ReviewEvent {
    fn from(e: ReviewEventParam) -> Self {
        match e {
            ReviewEventParam::Approve => aptu_core::ReviewEvent::Approve,
            ReviewEventParam::RequestChanges => aptu_core::ReviewEvent::RequestChanges,
            ReviewEventParam::Comment => aptu_core::ReviewEvent::Comment,
        }
    }
}

impl std::fmt::Display for ReviewEventParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewEventParam::Approve => write!(f, "approve"),
            ReviewEventParam::RequestChanges => write!(f, "request_changes"),
            ReviewEventParam::Comment => write!(f, "comment"),
        }
    }
}

/// Parameters for posting a PR review.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Post an AI review on a GitHub pull request")]
pub struct PostReviewParams {
    /// PR reference (e.g. "owner/repo#456" or full URL).
    #[schemars(description = "PR reference such as owner/repo#456 or a GitHub URL")]
    pub pr_ref: String,
    /// Review event type.
    #[schemars(description = "Review action: approve, request_changes, or comment")]
    pub event: ReviewEventParam,
}

/// Credential validation status.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum CredentialStatus {
    /// Credential is valid and working.
    Valid,
    /// Credential is missing or not set.
    Missing,
    /// Credential is invalid or non-functional.
    Invalid,
}

/// Health check response with credential validation results.
#[derive(Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct HealthCheckResponse {
    /// GitHub token validation status.
    pub github_token: CredentialStatus,
    /// AI API key presence status.
    pub ai_api_key: CredentialStatus,
}

/// Parameters for health check (empty for consistency).
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Check the health of credentials and configuration")]
pub struct HealthCheckParams {}

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

/// MCP server exposing aptu-core functionality.
#[derive(Clone)]
pub struct AptuServer {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
    ai_config: aptu_core::config::AiConfig,
}

impl Default for AptuServer {
    fn default() -> Self {
        Self::new(false)
    }
}

// ---------------------------------------------------------------------------
// Tools (generates Self::tool_router())
// ---------------------------------------------------------------------------

#[tool_router]
impl AptuServer {
    /// Create a new `AptuServer` with custom AI configuration.
    ///
    /// # Arguments
    /// * `read_only` - If true, disables write tools (`post_triage`, `post_review`)
    /// * `ai_config` - AI provider configuration to use for all tool handlers
    #[must_use]
    pub fn with_config(read_only: bool, ai_config: aptu_core::config::AiConfig) -> Self {
        let mut tool_router = Self::tool_router();

        if read_only {
            tool_router.remove_route("post_triage");
            tool_router.remove_route("post_review");
            tracing::info!(
                "Read-only mode enabled: write tools disabled (post_triage, post_review)"
            );
        }

        Self {
            tool_router,
            prompt_router: Self::prompt_router(),
            ai_config,
        }
    }

    /// Create a new `AptuServer` with default AI configuration.
    ///
    /// This is a backward-compatible wrapper around `with_config()` that uses
    /// `AiConfig::default()`. For custom configuration, use `with_config()` instead.
    ///
    /// # Arguments
    /// * `read_only` - If true, disables write tools (`post_triage`, `post_review`)
    #[must_use]
    pub fn new(read_only: bool) -> Self {
        Self::with_config(read_only, aptu_core::config::AiConfig::default())
    }

    #[tool(
        name = "triage_issue",
        description = "Fetch a GitHub issue and run AI triage analysis. Returns analysis only, without posting anything to GitHub; call post_triage to publish the result. Returns a JSON object with fields: summary, suggested_labels, clarifying_questions, potential_duplicates, related_issues, contributor_guidance. issue_ref format: owner/repo#123 or a full GitHub issue URL (e.g. https://github.com/owner/repo/issues/123). Requires GITHUB_TOKEN and an AI API key in the environment.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<aptu_core::ai::types::TriageResponse>(),
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = true)
    )]
    async fn triage_issue(
        &self,
        Parameters(params): Parameters<TriageIssueParams>,
    ) -> Result<CallToolResult, McpError> {
        let provider = EnvTokenProvider;
        let ai_config = self.ai_config.clone();

        let issue = aptu_core::facade::fetch_issue_for_triage(&provider, &params.issue_ref, None)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let response = aptu_core::facade::analyze_issue(&provider, &issue, &ai_config)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let json = serde_json::to_string_pretty(&response.triage).map_err(generic_to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        name = "review_pr",
        description = "Fetch a GitHub pull request and run AI code review analysis. Returns analysis only, without posting anything to GitHub; call post_review to publish the result. Returns a JSON object with fields: summary, verdict, strengths, concerns, comments (array of {file, line, severity, comment}), suggestions. pr_ref format: owner/repo#456 or a full GitHub PR URL (e.g. https://github.com/owner/repo/pull/456). Requires GITHUB_TOKEN and an AI API key in the environment.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<aptu_core::ai::types::PrReviewResponse>(),
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = true)
    )]
    async fn review_pr(
        &self,
        Parameters(params): Parameters<ReviewPrParams>,
    ) -> Result<CallToolResult, McpError> {
        let provider = EnvTokenProvider;
        let ai_config = self.ai_config.clone();

        let pr = aptu_core::facade::fetch_pr_for_review(&provider, &params.pr_ref, None)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let (review, _stats) = aptu_core::facade::analyze_pr(&provider, &pr, &ai_config)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let json = serde_json::to_string_pretty(&review).map_err(generic_to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        name = "scan_security",
        description = "Scan a unified diff for security vulnerabilities and secrets without making API calls or running AI inference. Returns structured JSON findings. Use alongside review_pr for full coverage: scan_security detects patterns locally, review_pr provides AI-powered contextual analysis.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<Vec<aptu_core::security::types::Finding>>(),
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn scan_security(
        &self,
        Parameters(params): Parameters<ScanSecurityParams>,
    ) -> Result<CallToolResult, McpError> {
        let scanner = aptu_core::security::SecurityScanner::new();
        let findings = scanner.scan_diff(&params.diff);

        let json = serde_json::to_string_pretty(&findings).map_err(generic_to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        name = "post_triage",
        description = "Fetch a GitHub issue, run AI triage analysis, and post the result as a new comment on the issue. Writes to GitHub (creates a new comment; cannot be undone). Call triage_issue first to preview the analysis before committing. Calling this twice on the same issue posts duplicate comments. Returns a plain-text confirmation with the issue ref on success. issue_ref format: owner/repo#123 or a full GitHub issue URL. Requires GITHUB_TOKEN (with issue comment write permission) and an AI API key.",
        annotations(
            destructive_hint = true,
            open_world_hint = true,
            idempotent_hint = false
        )
    )]
    async fn post_triage(
        &self,
        Parameters(params): Parameters<PostTriageParams>,
    ) -> Result<CallToolResult, McpError> {
        let provider = EnvTokenProvider;
        let ai_config = self.ai_config.clone();

        let issue = aptu_core::facade::fetch_issue_for_triage(&provider, &params.issue_ref, None)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let response = aptu_core::facade::analyze_issue(&provider, &issue, &ai_config)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        aptu_core::facade::post_triage_comment(&provider, &issue, &response.triage)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Triage comment posted on {}",
            params.issue_ref
        ))]))
    }

    #[tool(
        name = "post_review",
        description = "Fetch a GitHub pull request, run AI code review analysis, and submit the result as a GitHub review. Writes to GitHub (submits a review; cannot be undone). Call review_pr first to inspect the analysis before committing. event controls the review outcome: approve submits an approval, request_changes blocks merging until resolved, comment posts feedback without a merge decision. Calling this twice on the same PR submits duplicate reviews. Returns a plain-text confirmation with the PR ref and event type on success. pr_ref format: owner/repo#456 or a full GitHub PR URL. Requires GITHUB_TOKEN (with PR review write permission) and an AI API key.",
        annotations(
            destructive_hint = true,
            open_world_hint = true,
            idempotent_hint = false
        )
    )]
    async fn post_review(
        &self,
        Parameters(params): Parameters<PostReviewParams>,
    ) -> Result<CallToolResult, McpError> {
        let provider = EnvTokenProvider;
        let ai_config = self.ai_config.clone();

        let pr = aptu_core::facade::fetch_pr_for_review(&provider, &params.pr_ref, None)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let (review, _stats) = aptu_core::facade::analyze_pr(&provider, &pr, &ai_config)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        let event = params.event.into();

        aptu_core::facade::post_pr_review(&provider, &params.pr_ref, None, &review.summary, event)
            .await
            .map_err(|e| aptu_error_to_mcp(&e))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Review posted on {} with event: {}",
            params.pr_ref, params.event
        ))]))
    }

    #[tool(
        name = "health",
        description = "Check GitHub token format and AI API key presence. Token validation is format-only (prefix matching: ghp_, gho_, ghu_, ghs_, ghr_, github_pat_) -- does not make a live API call. Returns a JSON object with fields github_token and ai_api_key, each set to Valid, Missing, or Invalid. Call at session start before running analysis tools.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<HealthCheckResponse>(),
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn health(
        &self,
        Parameters(_params): Parameters<HealthCheckParams>,
    ) -> Result<CallToolResult, McpError> {
        let provider = EnvTokenProvider;

        // Check GitHub token presence and format
        let github_token_status = match provider.github_token() {
            None => CredentialStatus::Missing,
            Some(token) => {
                let token_str = token.expose_secret();
                if token_str.is_empty() {
                    CredentialStatus::Missing
                } else if Self::is_valid_github_token_format(token_str) {
                    CredentialStatus::Valid
                } else {
                    CredentialStatus::Invalid
                }
            }
        };

        // Check AI API key presence
        let ai_api_key_status = match provider.ai_api_key("gemini") {
            None => CredentialStatus::Missing,
            Some(key) => {
                let key_str = key.expose_secret();
                if key_str.is_empty() {
                    CredentialStatus::Missing
                } else {
                    CredentialStatus::Valid
                }
            }
        };

        let response = HealthCheckResponse {
            github_token: github_token_status,
            ai_api_key: ai_api_key_status,
        };

        let json = serde_json::to_string_pretty(&response).map_err(generic_to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Validate GitHub token format without making API calls.
    ///
    /// Checks for known GitHub token prefixes:
    /// - `ghp_` - Personal Access Tokens
    /// - `gho_` - OAuth Access Tokens
    /// - `ghu_` - User-to-Server Tokens
    /// - `ghs_` - Server-to-Server Tokens
    /// - `ghr_` - Refresh Tokens
    /// - `github_pat_` - Fine-grained Personal Access Tokens (93 chars)
    #[must_use]
    pub fn is_valid_github_token_format(token: &str) -> bool {
        token.starts_with("ghp_")
            || token.starts_with("gho_")
            || token.starts_with("ghu_")
            || token.starts_with("ghs_")
            || token.starts_with("ghr_")
            || token.starts_with("github_pat_")
    }
}

// ---------------------------------------------------------------------------
// Prompts (generates Self::prompt_router())
// ---------------------------------------------------------------------------

#[prompt_router]
impl AptuServer {
    #[prompt(
        name = "triage_guide",
        description = "Step-by-step guide for triaging a GitHub issue"
    )]
    async fn triage_guide(&self) -> Result<Vec<PromptMessage>, McpError> {
        Ok(vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                "I need to triage a GitHub issue. Walk me through the process.",
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "Here is a step-by-step triage workflow:\n\n\
                 1. Read the issue title, body, and any linked references\n\
                 2. Check for reproducibility information and environment details\n\
                 3. Assess severity: critical (data loss, security), high (broken feature), \
                    medium (degraded experience), low (cosmetic, minor)\n\
                 4. Identify the affected component or module\n\
                 5. Check for duplicates using search\n\
                 6. Apply appropriate labels (bug, enhancement, documentation, etc.)\n\
                 7. Estimate complexity: simple (< 1 day), medium (1-3 days), complex (> 3 days)\n\
                 8. Add to the relevant milestone if applicable\n\
                 9. Write a triage summary comment with your assessment\n\n\
                 Two-step workflow for AI-assisted triage:\n\n\
                 Step 1: Call `triage_issue` with the issue reference to fetch and analyze the \
                 issue. This is read-only; nothing is posted to GitHub.\n\n\
                 Step 2: Review the analysis returned by `triage_issue`.\n\n\
                 Step 3: If satisfied, call `post_triage` with the same issue reference to \
                 publish the triage comment. This is destructive and cannot be undone. \
                 Calling `post_triage` twice on the same issue posts duplicate comments.",
            ),
        ])
    }

    #[prompt(
        name = "review_checklist",
        description = "Checklist for reviewing a GitHub pull request"
    )]
    async fn review_checklist(&self) -> Result<Vec<PromptMessage>, McpError> {
        Ok(vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                "I need to review a pull request. Give me a checklist.",
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "PR Review Checklist:\n\n\
                 Code Quality:\n\
                 - [ ] Code follows project style and conventions\n\
                 - [ ] No unnecessary complexity (KISS/YAGNI)\n\
                 - [ ] No code duplication (DRY)\n\
                 - [ ] Error handling is appropriate\n\
                 - [ ] No hardcoded values that should be configurable\n\n\
                 Testing:\n\
                 - [ ] Tests cover the changes adequately\n\
                 - [ ] Edge cases are handled\n\
                 - [ ] Tests pass locally\n\n\
                 Security:\n\
                 - [ ] No secrets or credentials in code\n\
                 - [ ] Input validation is present\n\
                 - [ ] No SQL injection or XSS vulnerabilities\n\n\
                 Documentation:\n\
                 - [ ] Public APIs are documented\n\
                 - [ ] Breaking changes are noted\n\
                 - [ ] CHANGELOG updated if needed\n\n\
                 To use `scan_security`, first obtain a unified diff: run \
                 `git diff <base-branch>` or `git diff --staged` locally and pass \
                 the output as the `diff` parameter.\n\n\
                 Use the `review_pr` tool for AI analysis, `scan_security` to check \
                 for vulnerabilities, then `post_review` to submit your review.",
            ),
        ])
    }
}

// ---------------------------------------------------------------------------
// Resources (manual - no macro support in RMCP yet, see rust-sdk#337)
// ---------------------------------------------------------------------------

/// Build the list of available MCP resources.
fn resource_list() -> Vec<Resource> {
    let mut repos = RawResource::new("aptu://repos", "Curated Repositories");
    repos.description = Some("List of curated open-source repositories for triage".into());
    repos.mime_type = Some("application/json".into());

    let mut issues = RawResource::new("aptu://issues", "Good First Issues");
    issues.description = Some("Good first issues from curated repositories".into());
    issues.mime_type = Some("application/json".into());

    let mut config = RawResource::new("aptu://config", "Configuration");
    config.description = Some("Current aptu configuration settings".into());
    config.mime_type = Some("application/json".into());

    vec![
        repos.no_annotation(),
        issues.no_annotation(),
        config.no_annotation(),
    ]
}

/// Build the list of resource templates (parameterized URIs).
fn resource_template_list() -> Vec<ResourceTemplate> {
    vec![
        RawResourceTemplate {
            uri_template: "aptu://repos/{owner}/{name}".into(),
            name: "Repository Detail".into(),
            title: None,
            description: Some("Details for a specific curated repository".into()),
            mime_type: Some("application/json".into()),
            icons: None,
        }
        .no_annotation(),
    ]
}

/// Read a resource by URI, dispatching to the appropriate handler.
async fn read_resource_by_uri(uri: &str) -> Result<ReadResourceResult, McpError> {
    // Match static resources first, then templates
    match uri {
        "aptu://repos" => {
            let repos = aptu_core::facade::list_curated_repos()
                .await
                .map_err(|e| aptu_error_to_mcp(&e))?;
            let json = serde_json::to_string_pretty(&repos).map_err(generic_to_mcp_error)?;
            Ok(ReadResourceResult::new(vec![ResourceContents::text(
                json, uri,
            )]))
        }
        "aptu://issues" => {
            let provider = EnvTokenProvider;
            let issues = aptu_core::facade::fetch_issues(&provider, None, true)
                .await
                .map_err(|e| aptu_error_to_mcp(&e))?;
            let json = serde_json::to_string_pretty(&issues).map_err(generic_to_mcp_error)?;
            Ok(ReadResourceResult::new(vec![ResourceContents::text(
                json, uri,
            )]))
        }
        "aptu://config" => {
            let config = aptu_core::config::load_config().map_err(|e| aptu_error_to_mcp(&e))?;
            let text = serde_json::to_string_pretty(&config).map_err(generic_to_mcp_error)?;
            Ok(ReadResourceResult::new(vec![ResourceContents::text(
                text, uri,
            )]))
        }
        _ => {
            // Try template: aptu://repos/{owner}/{name}
            if let Some(path) = uri.strip_prefix("aptu://repos/") {
                let parts: Vec<&str> = path.splitn(2, '/').collect();
                if parts.len() == 2 {
                    let (owner, name) = (parts[0], parts[1]);
                    let repos = aptu_core::facade::list_curated_repos()
                        .await
                        .map_err(|e| aptu_error_to_mcp(&e))?;
                    let repo = repos
                        .iter()
                        .find(|r| {
                            r.owner.eq_ignore_ascii_case(owner) && r.name.eq_ignore_ascii_case(name)
                        })
                        .ok_or_else(|| {
                            McpError::resource_not_found(
                                "resource_not_found",
                                Some(serde_json::json!({ "uri": uri })),
                            )
                        })?;
                    let json = serde_json::to_string_pretty(repo).map_err(generic_to_mcp_error)?;
                    return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                        json, uri,
                    )]));
                }
            }
            Err(McpError::resource_not_found(
                "resource_not_found",
                Some(serde_json::json!({ "uri": uri })),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// ServerHandler (combines tool_handler + prompt_handler + manual resources)
// ---------------------------------------------------------------------------

#[tool_handler]
#[prompt_handler]
impl ServerHandler for AptuServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
        )
        .with_instructions(
            "Aptu MCP server for AI-powered GitHub issue triage and pull request review. \
             Use triage_issue to analyze an issue and review_pr to analyze a PR; both are read-only and return analysis only. \
             Call post_triage or post_review to publish results to GitHub -- these are destructive and cannot be undone; they are absent in read-only mode. \
             scan_security scans a unified diff locally without any AI call, complementing review_pr. \
             Call health at session start to validate your GitHub token format and AI API key presence before running analysis tools. \
             Resources: repos (curated repository list), issues (good first issues), config (current configuration). \
             Prompts: triage_guide and review_checklist provide step-by-step guided workflows.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: resource_list(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        read_resource_by_uri(request.uri.as_str()).await
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: resource_template_list(),
            next_cursor: None,
            meta: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_info_has_all_capabilities() {
        let server = AptuServer::new(false);
        let info = server.get_info();
        let caps = &info.capabilities;
        assert!(caps.tools.is_some());
        assert!(caps.prompts.is_some());
        assert!(caps.resources.is_some());
    }

    #[test]
    fn server_info_has_instructions() {
        let server = AptuServer::new(false);
        let info = server.get_info();
        assert!(info.instructions.is_some());
        let instructions = info.instructions.unwrap();
        assert!(instructions.contains("triage_issue"));
        assert!(instructions.contains("review_pr"));
    }

    #[test]
    fn resource_list_has_three_entries() {
        let resources = resource_list();
        assert_eq!(resources.len(), 3);
    }

    #[test]
    fn resource_list_uris_are_valid() {
        let resources = resource_list();
        let uris: Vec<&str> = resources.iter().map(|r| r.raw.uri.as_str()).collect();
        assert!(uris.contains(&"aptu://repos"));
        assert!(uris.contains(&"aptu://issues"));
        assert!(uris.contains(&"aptu://config"));
    }

    #[test]
    fn resource_list_has_mime_types() {
        let resources = resource_list();
        for resource in &resources {
            let mime = resource.raw.mime_type.as_deref().unwrap();
            assert_eq!(
                mime, "application/json",
                "all resources should have mime_type = application/json, got {mime} for {}",
                resource.uri
            );
        }
    }

    #[test]
    fn resource_template_list_has_repo_detail() {
        let templates = resource_template_list();
        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates[0].raw.uri_template.as_str(),
            "aptu://repos/{owner}/{name}"
        );
    }

    #[test]
    fn tool_router_has_six_tools() {
        let router = AptuServer::tool_router();
        assert_eq!(router.list_all().len(), 6);
    }

    #[test]
    fn tool_router_tool_names() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"triage_issue"));
        assert!(names.contains(&"review_pr"));
        assert!(names.contains(&"scan_security"));
        assert!(names.contains(&"post_triage"));
        assert!(names.contains(&"post_review"));
        assert!(names.contains(&"health"));
    }

    #[test]
    fn prompt_router_has_two_prompts() {
        let router = AptuServer::prompt_router();
        assert_eq!(router.list_all().len(), 2);
    }

    #[test]
    fn prompt_router_prompt_names() {
        let router = AptuServer::prompt_router();
        let prompts = router.list_all();
        let names: Vec<&str> = prompts.iter().map(|p| p.name.as_ref()).collect();
        assert!(names.contains(&"triage_guide"));
        assert!(names.contains(&"review_checklist"));
    }

    #[test]
    fn read_only_tools_have_annotation() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        for tool in &tools {
            let name: &str = tool.name.as_ref();
            if let Some(ref annotations) = tool.annotations {
                match name {
                    "triage_issue" | "review_pr" | "scan_security" => {
                        assert_eq!(annotations.read_only_hint, Some(true));
                    }
                    "post_triage" | "post_review" => {
                        assert_eq!(annotations.destructive_hint, Some(true));
                    }
                    _ => {}
                }
            }
        }
    }

    #[tokio::test]
    async fn read_resource_unknown_uri_returns_error() {
        let result = read_resource_by_uri("aptu://unknown").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_resource_invalid_repo_path_returns_error() {
        let result = read_resource_by_uri("aptu://repos/").await;
        assert!(result.is_err());
    }

    #[test]
    fn triage_issue_params_schema() {
        let schema = schemars::schema_for!(TriageIssueParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("properties").is_some());
    }

    #[test]
    fn review_pr_params_schema() {
        let schema = schemars::schema_for!(ReviewPrParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("properties").is_some());
    }

    #[test]
    fn scan_security_params_schema() {
        let schema = schemars::schema_for!(ScanSecurityParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("properties").is_some());
    }

    #[test]
    fn post_triage_params_schema() {
        let schema = schemars::schema_for!(PostTriageParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("properties").is_some());
    }

    #[test]
    fn post_review_params_schema() {
        let schema = schemars::schema_for!(PostReviewParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json["properties"].get("event").is_some());

        // Event is a $ref to ReviewEventParam in $defs, need to check the definition
        let defs = &json["$defs"];
        assert!(defs.get("ReviewEventParam").is_some());

        let event_param_schema = &defs["ReviewEventParam"];
        // Verify it uses oneOf with const values
        assert!(event_param_schema.get("oneOf").is_some());

        let one_of = &event_param_schema["oneOf"];
        assert!(one_of.is_array());
        let one_of_arr = one_of.as_array().unwrap();
        assert_eq!(one_of_arr.len(), 3);

        // Extract the const values
        let const_values: Vec<&str> = one_of_arr
            .iter()
            .filter_map(|v| v.get("const").and_then(|c| c.as_str()))
            .collect();
        assert_eq!(const_values, vec!["approve", "request_changes", "comment"]);
    }

    #[test]
    fn review_event_param_rejects_invalid_value() {
        let result = serde_json::from_str::<ReviewEventParam>("\"invalid_event\"");
        assert!(result.is_err());
    }

    #[test]
    fn health_check_params_schema() {
        let schema = schemars::schema_for!(HealthCheckParams);
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("type").is_some());
    }

    #[test]
    fn credential_status_serializes_to_pascalcase() {
        let valid = serde_json::to_string(&CredentialStatus::Valid).unwrap();
        assert_eq!(valid, "\"Valid\"");

        let missing = serde_json::to_string(&CredentialStatus::Missing).unwrap();
        assert_eq!(missing, "\"Missing\"");

        let invalid = serde_json::to_string(&CredentialStatus::Invalid).unwrap();
        assert_eq!(invalid, "\"Invalid\"");
    }

    #[test]
    fn health_check_response_serializes_correctly() {
        let response = HealthCheckResponse {
            github_token: CredentialStatus::Valid,
            ai_api_key: CredentialStatus::Missing,
        };

        let json = serde_json::to_string_pretty(&response).unwrap();
        assert!(json.contains("github_token"));
        assert!(json.contains("Valid"));
        assert!(json.contains("ai_api_key"));
        assert!(json.contains("Missing"));
    }
    #[test]
    fn health_tool_has_read_only_annotation() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let health_tool = tools.iter().find(|t| t.name == "health").unwrap();
        assert_eq!(
            health_tool.annotations.as_ref().unwrap().read_only_hint,
            Some(true)
        );
        assert_eq!(
            health_tool.annotations.as_ref().unwrap().idempotent_hint,
            Some(true)
        );
    }

    #[test]
    fn tool_output_schemas_present() {
        let router = AptuServer::tool_router();
        let tools: Vec<_> = router.list_all();
        let schema_tools = ["triage_issue", "review_pr", "scan_security", "health"];
        for name in schema_tools {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            assert!(
                tool.output_schema.is_some(),
                "tool {name} missing output_schema"
            );
        }
    }

    #[test]
    fn read_only_false_includes_all_tools() {
        let server = AptuServer::new(false);
        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"post_triage"));
        assert!(names.contains(&"post_review"));
    }

    #[test]
    fn read_only_true_removes_write_tools() {
        let server = AptuServer::new(true);
        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.len(), 4);
        assert!(!names.contains(&"post_triage"));
        assert!(!names.contains(&"post_review"));
        assert!(names.contains(&"triage_issue"));
        assert!(names.contains(&"review_pr"));
        assert!(names.contains(&"scan_security"));
        assert!(names.contains(&"health"));
    }

    #[test]
    fn with_config_stores_custom_config() {
        let custom_config = aptu_core::config::AiConfig {
            provider: "custom-provider".to_string(),
            model: "custom-model".to_string(),
            ..Default::default()
        };

        let server = AptuServer::with_config(false, custom_config.clone());
        assert_eq!(server.ai_config.provider, "custom-provider");
        assert_eq!(server.ai_config.model, "custom-model");
    }

    #[test]
    fn new_wraps_with_config_default() {
        let server = AptuServer::new(false);
        let default_config = aptu_core::config::AiConfig::default();
        assert_eq!(server.ai_config.provider, default_config.provider);
        assert_eq!(server.ai_config.model, default_config.model);
    }

    #[test]
    fn triage_issue_description_is_read_only() {
        let server = AptuServer::new(false);
        let tools = server.tool_router.list_all();
        let triage_issue = tools
            .iter()
            .find(|t| t.name == "triage_issue")
            .expect("triage_issue tool not found");
        assert!(
            triage_issue
                .description
                .as_ref()
                .map(|d| d.contains("Returns analysis only"))
                .unwrap_or(false),
            "triage_issue description should indicate read-only nature"
        );
    }

    #[test]
    fn review_pr_description_is_read_only() {
        let server = AptuServer::new(false);
        let tools = server.tool_router.list_all();
        let review_pr = tools
            .iter()
            .find(|t| t.name == "review_pr")
            .expect("review_pr tool not found");
        assert!(
            review_pr
                .description
                .as_ref()
                .map(|d| d.contains("Returns analysis only"))
                .unwrap_or(false),
            "review_pr description should indicate read-only nature"
        );
    }

    #[test]
    fn post_triage_description_warns_of_consequences() {
        let server = AptuServer::new(false);
        let tools = server.tool_router.list_all();
        let post_triage = tools
            .iter()
            .find(|t| t.name == "post_triage")
            .expect("post_triage tool not found");
        assert!(
            post_triage
                .description
                .as_ref()
                .map(|d| d.contains("cannot be undone"))
                .unwrap_or(false),
            "post_triage description should warn that the action cannot be undone"
        );
    }

    #[test]
    fn post_review_description_warns_of_consequences() {
        let server = AptuServer::new(false);
        let tools = server.tool_router.list_all();
        let post_review = tools
            .iter()
            .find(|t| t.name == "post_review")
            .expect("post_review tool not found");
        assert!(
            post_review
                .description
                .as_ref()
                .map(|d| d.contains("cannot be undone"))
                .unwrap_or(false),
            "post_review description should warn that the action cannot be undone"
        );
    }

    #[test]
    fn triage_issue_has_idempotent_hint() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let triage_issue = tools
            .iter()
            .find(|t| t.name == "triage_issue")
            .expect("triage_issue tool not found");
        assert_eq!(
            triage_issue.annotations.as_ref().unwrap().idempotent_hint,
            Some(true),
            "triage_issue should have idempotent_hint = true"
        );
    }

    #[test]
    fn review_pr_has_idempotent_hint() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let review_pr = tools
            .iter()
            .find(|t| t.name == "review_pr")
            .expect("review_pr tool not found");
        assert_eq!(
            review_pr.annotations.as_ref().unwrap().idempotent_hint,
            Some(true),
            "review_pr should have idempotent_hint = true"
        );
    }

    #[test]
    fn post_triage_has_idempotent_hint_false() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let post_triage = tools
            .iter()
            .find(|t| t.name == "post_triage")
            .expect("post_triage tool not found");
        assert_eq!(
            post_triage.annotations.as_ref().unwrap().idempotent_hint,
            Some(false),
            "post_triage should have idempotent_hint = false"
        );
    }

    #[test]
    fn post_review_has_idempotent_hint_false() {
        let router = AptuServer::tool_router();
        let tools = router.list_all();
        let post_review = tools
            .iter()
            .find(|t| t.name == "post_review")
            .expect("post_review tool not found");
        assert_eq!(
            post_review.annotations.as_ref().unwrap().idempotent_hint,
            Some(false),
            "post_review should have idempotent_hint = false"
        );
    }

    #[test]
    fn config_resource_has_json_mime_type() {
        let resources = resource_list();
        let config_resource = resources
            .iter()
            .find(|r| r.uri == "aptu://config")
            .expect("aptu://config resource not found");
        assert_eq!(
            config_resource.mime_type,
            Some("application/json".into()),
            "aptu://config should have mime_type = application/json"
        );
    }
}
