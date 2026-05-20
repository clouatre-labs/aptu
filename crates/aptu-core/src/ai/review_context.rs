// SPDX-License-Identifier: Apache-2.0

//! Review context policy layer for PR analysis.
//!
//! Centralizes all enrichment decisions (AST context, call graph, dependency enrichments)
//! and CWD inference into a single `ReviewContext` struct and `build_review_context()` function.

use std::path::PathBuf;

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Review context containing all enrichment data and configuration for PR analysis.
///
/// This struct centralizes enrichment decisions and is passed to `build_pr_review_user_prompt()`
/// to avoid scattered conditional logic throughout the codebase.
#[derive(Clone)]
pub struct ReviewContext {
    /// Pull request details.
    pub pr: PrDetails,
    /// AST context for changed files (empty if not available or feature disabled).
    pub ast_context: String,
    /// Call graph context for changed files (empty if not available or feature disabled).
    pub call_graph: String,
    /// Inferred repository path from CWD (if available).
    pub inferred_repo_path: Option<PathBuf>,
    /// Whether the repository path was inferred from CWD.
    pub cwd_inferred: bool,
}

impl ReviewContext {
    /// Returns a formatted pre-flight summary for verbose output.
    ///
    /// Includes package names, character counts, and CWD inference status.
    #[must_use]
    pub fn verbose_summary(&self) -> String {
        use std::fmt::Write;

        let mut summary = String::new();

        // Repo path info
        if let Some(path) = &self.inferred_repo_path {
            let inferred_label = if self.cwd_inferred { " (inferred)" } else { "" };
            let _ = writeln!(
                summary,
                "Repository path: {}{}",
                path.display(),
                inferred_label
            );
        }

        // Enrichment summary
        if !self.pr.dep_enrichments.is_empty() {
            let packages: Vec<&str> = self
                .pr
                .dep_enrichments
                .iter()
                .map(|d| d.package_name.as_str())
                .collect();
            let _ = writeln!(summary, "Dependency enrichments: {}", packages.join(", "));
        }

        // Context sizes
        let mut context_sizes = Vec::new();
        if !self.ast_context.is_empty() {
            context_sizes.push(format!("AST: {} chars", self.ast_context.len()));
        }
        if !self.call_graph.is_empty() {
            context_sizes.push(format!("call graph: {} chars", self.call_graph.len()));
        }
        if !context_sizes.is_empty() {
            let _ = writeln!(summary, "Context: {}", context_sizes.join(", "));
        }

        summary
    }
}

/// Builds a `ReviewContext` by centralizing all enrichment decisions.
///
/// This function owns:
/// - CWD inference logic (moved from `facade.rs`)
/// - AST context building (moved from `facade.rs`)
/// - Call graph auto-enable logic (moved from `review_pr()`)
/// - Dependency enrichment (moved from `review_pr()`)
/// - Budget drop order enforcement
///
/// # Arguments
///
/// * `pr` - Pull request details
/// * `repo_path` - Optional explicit repository path (overrides CWD inference)
/// * `deep` - Whether to enable deep analysis (call graph)
/// * `review_config` - Review configuration with budget thresholds
///
/// # Returns
///
/// A `ReviewContext` with all enrichment fields populated according to budget constraints.
#[allow(clippy::too_many_lines)]
pub async fn build_review_context(
    mut pr: PrDetails,
    repo_path: Option<String>,
    deep: bool,
    review_config: &ReviewConfig,
) -> crate::Result<ReviewContext> {
    // Step 1: Infer repo_path from CWD if not provided
    let (inferred_repo_path, cwd_inferred) = if repo_path.is_none() {
        if let Some(inferred_path) = infer_repo_path_from_cwd(&pr.owner, &pr.repo) {
            (Some(PathBuf::from(&inferred_path)), true)
        } else {
            (None, false)
        }
    } else {
        (repo_path.map(PathBuf::from), false)
    };

    let repo_path_ref = inferred_repo_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    // Step 2: Build AST context if repo_path resolved
    let ast_context = build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await;

    // Step 3: Enrich with dependency release notes
    pr.dep_enrichments = crate::ai::dep_enrichment::enrich_dep_releases(
        &pr.files,
        review_config.max_dep_packages,
        review_config.max_dep_release_chars,
    )
    .await;

    // Step 4: Estimate total chars and decide call_graph budget
    let mut estimated_size = estimate_pr_size(&pr, &ast_context);
    let max_prompt_chars = review_config.max_prompt_chars;

    // Auto-enable call graph if remaining budget is sufficient
    let size_without_call_graph = estimated_size.saturating_sub(0); // placeholder for call_graph size
    let remaining_budget = max_prompt_chars.saturating_sub(size_without_call_graph);
    let should_auto_enable_call_graph = remaining_budget > review_config.min_budget_for_call_graph;

    // Step 5: Build call_graph if decided
    let mut call_graph = if deep || should_auto_enable_call_graph {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files, true).await
    } else {
        String::new()
    };

    // Step 6: Apply budget drop order (call_graph -> ast_context -> dep_enrichments -> patches)
    estimated_size = estimate_pr_size(&pr, &ast_context);
    if !call_graph.is_empty() {
        estimated_size += call_graph.len();
    }

    // Drop call_graph if over budget (unless auto-enabled)
    if estimated_size > max_prompt_chars && !should_auto_enable_call_graph {
        tracing::warn!(
            section = "call_graph",
            chars = call_graph.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = call_graph.len();
        call_graph.clear();
        estimated_size -= dropped_chars;
    }

    // Drop ast_context if still over budget
    let mut ast_context = ast_context;
    if estimated_size > max_prompt_chars {
        tracing::warn!(
            section = "ast_context",
            chars = ast_context.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = ast_context.len();
        ast_context.clear();
        estimated_size -= dropped_chars;
    }

    // Drop dep_enrichments if still over budget
    if estimated_size > max_prompt_chars {
        let dropped_chars: usize = pr
            .dep_enrichments
            .iter()
            .map(|d| d.body.len() + d.package_name.len() + d.github_url.len())
            .sum();
        if dropped_chars > 0 {
            tracing::warn!(
                section = "dep_enrichments",
                chars = dropped_chars,
                "Dropping section: prompt budget exceeded"
            );
            pr.dep_enrichments.clear();
            estimated_size -= dropped_chars;
        }
    }

    // Drop largest file patches first if still over budget
    if estimated_size > max_prompt_chars {
        let mut file_sizes: Vec<(usize, usize)> = pr
            .files
            .iter()
            .enumerate()
            .map(|(idx, f)| (idx, f.patch.as_ref().map_or(0, String::len)))
            .collect();
        file_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

        for (file_idx, patch_size) in file_sizes {
            if estimated_size <= max_prompt_chars {
                break;
            }
            if patch_size > 0 {
                tracing::warn!(
                    file = %pr.files[file_idx].filename,
                    patch_chars = patch_size,
                    "Dropping patch: prompt budget exceeded"
                );
                pr.files[file_idx].patch = None;
                estimated_size -= patch_size;
            }
        }
    }

    // Drop full_content if still over budget
    if estimated_size > max_prompt_chars {
        let mut full_content_sizes: Vec<(usize, usize)> = pr
            .files
            .iter()
            .enumerate()
            .map(|(idx, f)| (idx, f.full_content.as_ref().map_or(0, String::len)))
            .collect();
        full_content_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

        for (file_idx, content_size) in full_content_sizes {
            if estimated_size <= max_prompt_chars {
                break;
            }
            if content_size > 0 {
                tracing::warn!(
                    file = %pr.files[file_idx].filename,
                    content_chars = content_size,
                    "Dropping full_content: prompt budget exceeded"
                );
                pr.files[file_idx].full_content = None;
                estimated_size -= content_size;
            }
        }
    }

    Ok(ReviewContext {
        pr,
        ast_context,
        call_graph,
        inferred_repo_path,
        cwd_inferred,
    })
}

/// Estimates the total character size of a PR review prompt.
fn estimate_pr_size(pr: &PrDetails, ast_context: &str) -> usize {
    let mut size = 0;

    // PR metadata
    size += pr.title.len() + pr.body.len() + pr.head_branch.len() + pr.base_branch.len();

    // Files and patches
    for file in &pr.files {
        size += file.filename.len() + file.status.len();
        if let Some(patch) = &file.patch {
            size += patch.len();
        }
        if let Some(content) = &file.full_content {
            size += content.len();
        }
    }

    // Enrichments
    for dep in &pr.dep_enrichments {
        size += dep.package_name.len() + dep.body.len() + dep.github_url.len();
    }

    // Context
    size += ast_context.len();

    size
}

/// Builds AST context for changed files.
async fn build_ctx_ast(repo_path: Option<&str>, files: &[crate::ai::types::PrFile]) -> String {
    let Some(path) = repo_path else {
        return String::new();
    };
    #[cfg(feature = "ast-context")]
    {
        return crate::ast_context::build_ast_context(path, files).await;
    }
    #[cfg(not(feature = "ast-context"))]
    {
        let _ = (path, files);
        String::new()
    }
}

/// Builds call-graph context for changed files.
async fn build_ctx_call_graph(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
    deep: bool,
) -> String {
    if !deep {
        return String::new();
    }
    let Some(path) = repo_path else {
        return String::new();
    };
    #[cfg(feature = "ast-context")]
    {
        return crate::ast_context::build_call_graph_context(path, files).await;
    }
    #[cfg(not(feature = "ast-context"))]
    {
        let _ = (path, files);
        String::new()
    }
}

/// Infers the repository path from the current working directory.
fn infer_repo_path_from_cwd(pr_owner: &str, pr_repo: &str) -> Option<String> {
    let git_root = get_git_root()?;
    let origin_url = get_git_origin_url()?;

    let Some((origin_owner, origin_repo)) = parse_origin_owner_repo(&origin_url) else {
        tracing::debug!(
            "infer_repo_path_from_cwd: parse_origin_owner_repo failed for {}",
            origin_url
        );
        return None;
    };

    let pr_owner_lower = pr_owner.to_lowercase();
    let pr_repo_lower = pr_repo.to_lowercase();

    if origin_owner == pr_owner_lower && origin_repo == pr_repo_lower {
        tracing::debug!(
            "infer_repo_path_from_cwd: matched origin {}/{} with PR {}/{}",
            origin_owner,
            origin_repo,
            pr_owner_lower,
            pr_repo_lower
        );
        Some(git_root)
    } else {
        tracing::debug!(
            "infer_repo_path_from_cwd: origin {}/{} does not match PR {}/{}",
            origin_owner,
            origin_repo,
            pr_owner_lower,
            pr_repo_lower
        );
        None
    }
}

/// Get git repository root directory.
fn get_git_root() -> Option<String> {
    use std::process::Command;

    Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

/// Get git origin URL.
fn get_git_origin_url() -> Option<String> {
    use std::process::Command;

    Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

/// Parses git remote URL to extract owner and repo.
fn parse_origin_owner_repo(url: &str) -> Option<(String, String)> {
    use crate::utils::parse_git_remote_url;

    let Ok(parsed) = parse_git_remote_url(url) else {
        return None;
    };

    let parts: Vec<&str> = parsed.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    let owner = parts[0].to_lowercase();
    let repo = parts[1].to_lowercase();
    Some((owner, repo))
}
