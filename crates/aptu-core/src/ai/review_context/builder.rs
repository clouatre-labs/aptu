// SPDX-License-Identifier: Apache-2.0

//! Review context assembly and enrichment.

use super::ReviewContext;
use super::budget::{apply_budget_drops, effective_budget, estimate_pr_size};
use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Builds a `ReviewContext` by centralizing all enrichment decisions.
///
/// This function owns:
/// - Dependency enrichment (moved from `review_pr()`)
/// - Budget drop order enforcement
///
/// # Arguments
///
/// * `pr` - Pull request details
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
    review_config: &ReviewConfig,
    context_window: Option<u32>,
    max_tokens: u32,
    custom_guidance: Option<&str>,
    prior_findings: Option<String>,
) -> crate::Result<ReviewContext> {
    // Step 1: Resolve the exact system prompt that `review_pr` will send so the
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

    let estimated_size = estimate_pr_size(&pr, system_overhead);

    // Step 2: Apply budget drop order
    let had_patches_pre_drop = pr
        .files
        .iter()
        .any(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()));
    let mut budget_drops = Vec::new();
    apply_budget_drops(
        &mut pr,
        max_prompt_chars,
        system_overhead,
        &mut budget_drops,
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
        estimated_size,
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
