// SPDX-License-Identifier: Apache-2.0

//! Review context assembly: CWD inference, AST/call-graph building, and enrichment.

use std::path::PathBuf;

use super::ReviewContext;
use super::budget::{
    apply_budget_drops, effective_budget, estimate_pr_size, should_enable_call_graph,
};
use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

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
/// * `review_config` - Review configuration with budget thresholds
/// * `context_window` - Optional model context window (tokens) capping the budget
/// * `max_tokens` - The request's output token budget; subtracted from the
///   context window before computing the input cap
/// * `custom_guidance` - Optional provider custom guidance included in the system
///   prompt, so the size estimate matches the actually-sent request
///
/// # Returns
///
/// A `ReviewContext` with all enrichment fields populated according to budget constraints.
#[allow(clippy::too_many_lines)]
pub async fn build_review_context(
    mut pr: PrDetails,
    repo_path: Option<String>,
    review_config: &ReviewConfig,
    context_window: Option<u32>,
    max_tokens: u32,
    custom_guidance: Option<&str>,
    prior_findings: Option<String>,
) -> crate::Result<ReviewContext> {
    // Step 1: Resolve repo_path (explicit or inferred from CWD)
    #[cfg(not(target_arch = "wasm32"))]
    let (inferred_repo_path, cwd_inferred) = resolve_repo_path(&pr, repo_path);
    #[cfg(target_arch = "wasm32")]
    let (inferred_repo_path, cwd_inferred) = (repo_path.map(std::path::PathBuf::from), false);
    let repo_path_ref = inferred_repo_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());

    // Step 2: Build AST context if repo_path resolved and opt-in enabled.
    // When `ast-context` feature is enabled, build_ctx_ast returns AstContextOutput
    // (which carries the text string); when disabled, it returns a plain String.
    #[cfg(feature = "ast-context")]
    let ast_context = if review_config.ast_context {
        build_ctx_ast(repo_path_ref.as_deref(), &pr.files)
            .await
            .text
    } else {
        String::new()
    };
    #[cfg(not(feature = "ast-context"))]
    let ast_context = if review_config.ast_context {
        build_ctx_ast(repo_path_ref.as_deref(), &pr.files).await
    } else {
        String::new()
    };

    // Step 3.5: Resolve the exact system prompt that `review_pr` will send so the
    // size estimate matches the actual request. This honors the provider's custom
    // guidance and the cfg-gated override file, mirroring the construction in
    // `provider/review.rs`.
    let guidance = crate::ai::context::load_custom_guidance(custom_guidance);
    #[cfg(not(target_arch = "wasm32"))]
    let system_prompt =
        match crate::ai::context::load_system_prompt_override("pr_review_system").await {
            Some(override_prompt) => override_prompt,
            None => crate::ai::prompts::build_pr_review_system_prompt(&guidance),
        };
    #[cfg(target_arch = "wasm32")]
    let system_prompt = crate::ai::prompts::build_pr_review_system_prompt(&guidance);
    let instructions_overhead = pr.instructions.as_deref().map_or(0, |instructions| {
        let escaped = instructions
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        format!("<repo_instructions>\n{escaped}\n</repo_instructions>\n\n").len()
    });
    let system_overhead = system_prompt.len() + instructions_overhead;

    // Effective budget caps max_prompt_chars at the model context window
    // minus the max_tokens output allowance (3.5 chars/token) when known;
    // None stays unbounded.
    let max_prompt_chars =
        effective_budget(review_config.max_prompt_chars, context_window, max_tokens);

    // The prior-findings summary is rendered into the user prompt, so its
    // section length (plus tags/headers overhead) must be part of the size
    // estimate and budget-drop math to keep the total within budget.
    let prior_findings_overhead = prior_findings
        .as_deref()
        .map_or(0, |summary| summary.len() + PRIOR_FINDINGS_SECTION_OVERHEAD);
    let system_overhead = system_overhead + prior_findings_overhead;

    // Step 4: Estimate total chars and decide call_graph budget
    // (call_graph not yet built, pass empty)
    let estimated_size = estimate_pr_size(&pr, &ast_context, "", system_overhead);
    let budget_remaining = max_prompt_chars.saturating_sub(estimated_size);

    // Step 4: Build call_graph if decided (only when AST enrichment is opt-in enabled)
    let should_enable_cg =
        review_config.ast_context && should_enable_call_graph(budget_remaining, review_config);
    let mut call_graph = if should_enable_cg {
        build_ctx_call_graph(repo_path_ref.as_deref(), &pr.files).await
    } else {
        String::new()
    };

    // Re-estimate with actual call_graph for accurate routing
    let final_estimated_size = estimate_pr_size(&pr, &ast_context, &call_graph, system_overhead);

    // Step 5: Apply budget drop order
    let had_patches_pre_drop = pr
        .files
        .iter()
        .any(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()));
    let mut ast_context = ast_context;
    let mut budget_drops = Vec::new();
    apply_budget_drops(
        &mut pr,
        &mut ast_context,
        &mut call_graph,
        max_prompt_chars,
        system_overhead,
        &mut budget_drops,
        repo_path_ref.as_deref(),
    );

    // Collect tracking metrics
    let files_total = pr.files.len();
    let files_with_patch = pr
        .files
        .iter()
        .filter(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()))
        .count();

    if had_patches_pre_drop && files_with_patch == 0 {
        tracing::warn!(
            pr_owner = %pr.owner,
            pr_repo = %pr.repo,
            pr_number = pr.number,
            files_total,
            files_with_patch,
            had_patches_pre_drop,
            "Review context has no surviving patches; refusing diff-less review"
        );
        return Err(crate::AptuError::EmptyReviewContext { files_total });
    }

    Ok(ReviewContext {
        pr,
        ast_context,
        call_graph,
        inferred_repo_path,
        cwd_inferred,
        max_chars_per_file: review_config.max_chars_per_file,
        max_diff_chars: review_config.max_diff_chars,
        max_patch_chars_per_file: review_config.max_patch_chars_per_file,
        files_truncated: 0,
        truncated_chars_dropped: 0,
        truncated_patch_files: Vec::new(),
        files_total,
        files_with_patch,
        budget_drops,
        prompt_chars_final: 0,
        estimated_size: final_estimated_size,
        context_window,
        prior_findings,
    })
}

/// Maximum characters for the prior-findings summary threaded into the prompt.
/// Kept well within existing prompt budgets; larger summaries are truncated.
pub(crate) const PRIOR_FINDINGS_MAX_CHARS: usize = 2_000;

/// Overhead for the prior-findings section tags/headers added by the prompt
/// builder; included in size estimates so budget math covers the full section.
pub(crate) const PRIOR_FINDINGS_SECTION_OVERHEAD: usize = 64;

/// Builds a short prior-findings summary from existing APTU review comments
/// (the dedup map source): one `path:line` entry per comment, first line only.
/// The result is capped to `PRIOR_FINDINGS_MAX_CHARS` so it never saturates
/// the prompt budget on PRs with many prior comments.
#[must_use]
pub fn prior_findings_summary(comments: &[crate::ai::types::PrReviewCommentDetails]) -> String {
    const MAX_LINE_CHARS: usize = 120;
    let mut summary = String::new();
    for comment in comments {
        let first_line = comment
            .body
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        if first_line.is_empty() {
            continue;
        }
        let first_line = truncate_at_char_boundary(first_line, MAX_LINE_CHARS);
        let location = match comment.line {
            Some(line) => format!("{}:{}", comment.path, line),
            None => comment.path.clone(),
        };
        let entry = format!("- {}: {}\n", location, first_line);
        if summary.len() + entry.len() > PRIOR_FINDINGS_MAX_CHARS {
            break;
        }
        summary.push_str(&entry);
    }
    summary
}

/// Truncates at a char boundary with an ellipsis marker.
fn truncate_at_char_boundary(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Resolves the repository path from explicit argument or CWD inference.
///
/// Returns a tuple of `(inferred_repo_path, cwd_inferred)`.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_repo_path(
    pr: &PrDetails,
    explicit_repo_path: Option<String>,
) -> (Option<PathBuf>, bool) {
    if explicit_repo_path.is_some() {
        (explicit_repo_path.map(PathBuf::from), false)
    } else if let Some(inferred_path) = infer_repo_path_from_cwd(&pr.owner, &pr.repo) {
        (Some(PathBuf::from(&inferred_path)), true)
    } else {
        (None, false)
    }
}

/// Builds AST context for changed files.
///
/// Returns an [`AstContextOutput`] containing the text string built from analysis.
#[allow(clippy::unused_async)]
#[cfg(feature = "ast-context")]
pub(crate) async fn build_ctx_ast(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> crate::ast_context::AstContextOutput {
    let Some(path) = repo_path else {
        return crate::ast_context::AstContextOutput::new(String::new());
    };
    crate::ast_context::build_ast_context(path, files).await
}

/// Builds AST context for changed files (stub when `ast-context` feature is off).
#[allow(clippy::unused_async)]
#[cfg(not(feature = "ast-context"))]
pub(crate) async fn build_ctx_ast(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> String {
    let _ = (repo_path, files);
    String::new()
}

/// Builds call-graph context for changed files.
#[allow(clippy::unused_async)]
pub(crate) async fn build_ctx_call_graph(
    repo_path: Option<&str>,
    files: &[crate::ai::types::PrFile],
) -> String {
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(test)]
mod prior_findings_tests {
    use super::prior_findings_summary;
    use crate::ai::types::PrReviewCommentDetails;

    fn comment(path: &str, line: Option<u64>, body: &str) -> PrReviewCommentDetails {
        PrReviewCommentDetails {
            id: 1,
            author: "aptu[bot]".to_string(),
            is_bot: true,
            body: body.to_string(),
            path: path.to_string(),
            line,
            side: Some("RIGHT".to_string()),
            commit_id: "abc".to_string(),
            original_line: None,
        }
    }

    #[test]
    fn test_summary_lists_path_and_first_line() {
        // Arrange: one prior finding with a multi-line body.
        let comments = vec![comment(
            "src/lib.rs",
            Some(15),
            "Possible unwrap panic\n\nMore detail here",
        )];

        // Act
        let summary = prior_findings_summary(&comments);

        // Assert
        assert!(summary.contains("src/lib.rs:15"));
        assert!(summary.contains("Possible unwrap panic"));
        assert!(!summary.contains("More detail"));
    }

    #[test]
    fn test_summary_omits_line_when_absent() {
        // Arrange: a prior finding with no line number.
        let comments = vec![comment("src/lib.rs", None, "Possible unwrap panic")];

        // Act
        let summary = prior_findings_summary(&comments);

        // Assert: path without a `:line` suffix.
        assert!(summary.contains("- src/lib.rs: Possible unwrap panic"));
    }

    #[test]
    fn test_summary_skips_blank_bodies() {
        // Arrange: one prior finding whose body is only whitespace.
        let comments = vec![comment("src/lib.rs", Some(15), "   \n\t\n")];

        // Act
        let summary = prior_findings_summary(&comments);

        // Assert: blank-bodied comments contribute no entry.
        assert!(summary.is_empty());
    }

    #[test]
    fn test_summary_capped_to_char_budget() {
        // Arrange: many long prior findings.
        let comments: Vec<_> = (0..500)
            .map(|i| {
                comment(
                    &format!("file{i}.rs"),
                    Some(1),
                    &format!("finding {} {}", i, "x".repeat(200)),
                )
            })
            .collect();

        // Act
        let summary = prior_findings_summary(&comments);

        // Assert: within the configured cap (plus per-entry overhead guard).
        assert!(summary.len() <= super::PRIOR_FINDINGS_MAX_CHARS + 300);
    }

    #[test]
    fn test_summary_empty_when_no_comments() {
        assert!(prior_findings_summary(&[]).is_empty());
    }
}
