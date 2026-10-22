// SPDX-License-Identifier: Apache-2.0

//! Prompt size estimation and budget-driven enrichment drops.

#[cfg(feature = "ast-context")]
use std::fmt::Write as _;

#[cfg(feature = "ast-context")]
use aptu_coder_core::{analyze_file, language_for_extension};

use crate::ai::types::PrDetails;
use crate::config::ReviewConfig;

/// Estimated overhead for XML tags, section headers, and schema preamble added by
/// `build_pr_review_user_prompt`. Used to ensure the prompt budget accounts for
/// non-content characters when estimating total prompt size.
pub(crate) const PROMPT_OVERHEAD_CHARS: usize = 1_000;

/// Estimated characters per token (in tenths, 35/10 = 3.5). Heuristic used to
/// convert a model's token-based context window into the character budget used
/// by prompt budgeting. This is intentionally a conservative approximation.
pub(crate) const CHARS_PER_TOKEN_TENTHS: usize = 35;

/// Minimal effective budget floor (characters). When the request's
/// `max_tokens` output allowance consumes the entire (or more than the entire)
/// model context window, the input cap saturates to this floor rather than
/// zero, leaving room for a minimal prompt instead of hard-failing every
/// request.
pub(crate) const MIN_EFFECTIVE_BUDGET_CHARS: usize = 1_000;

/// Computes the effective prompt character budget available for input.
///
/// Caps `max_prompt_chars` at `(context_window - max_tokens) * 3.5` when a
/// context window is known, reserving the request's `max_tokens` completion
/// budget so input plus output fits within the model's context. When
/// `max_tokens >= context_window`, the cap saturates to
/// [`MIN_EFFECTIVE_BUDGET_CHARS`]. A `None` context window is treated as
/// unbounded: `max_prompt_chars` is returned unchanged.
///
/// Note: the truncation-retry escalation in `provider/http.rs` can raise
/// `max_tokens` at request time; that escalated value is not knowable at
/// budget time, so the configured/requested initial `max_tokens` is used.
#[must_use]
pub(crate) fn effective_budget(
    max_prompt_chars: usize,
    context_window: Option<u32>,
    max_tokens: u32,
) -> usize {
    match context_window {
        None => max_prompt_chars,
        Some(window) => {
            // Saturating u64 math guards the window * 3.5 conversion against
            // overflow on 32-bit targets or absurd registry values.
            let input_window = u64::from(window).saturating_sub(u64::from(max_tokens));
            let cap =
                usize::try_from(input_window.saturating_mul(CHARS_PER_TOKEN_TENTHS as u64) / 10)
                    .unwrap_or(usize::MAX);
            max_prompt_chars.min(cap.max(MIN_EFFECTIVE_BUDGET_CHARS))
        }
    }
}

/// Determines whether to enable call graph context based on the remaining
/// prompt budget. Call-graph context is auto-enabled whenever the estimated
/// prompt leaves more than `min_budget_for_call_graph` characters available.
pub(crate) fn should_enable_call_graph(budget_remaining: usize, config: &ReviewConfig) -> bool {
    budget_remaining > config.min_budget_for_call_graph
}

/// Applies budget drop order: `call_graph` -> `ast_context` ->
/// `full_content` -> patches.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. `call_graph` -- dropped first
/// 2. `ast_context` -- dropped second
/// 3. file `full_content` -- dropped largest-first
/// 4. file patches -- dropped largest-first as last resort, so the diff itself
///    (the highest-value context for a review) is preserved as long as possible
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
pub(crate) fn apply_budget_drops(
    pr: &mut PrDetails,
    ast_context: &mut String,
    call_graph: &mut String,
    max_prompt_chars: usize,
    system_overhead: usize,
    budget_drops: &mut Vec<String>,
    repo_path: Option<&str>,
) {
    let mut estimated_size = estimate_pr_size(pr, ast_context, call_graph, system_overhead);

    // Drop call_graph if over budget
    if estimated_size > max_prompt_chars {
        tracing::warn!(
            section = "call_graph",
            chars = call_graph.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = call_graph.len();
        call_graph.clear();
        estimated_size -= dropped_chars;
        if dropped_chars > 0 {
            budget_drops.push("call_graph".to_string());
        }
    }

    // Drop ast_context if still over budget
    if estimated_size > max_prompt_chars {
        tracing::warn!(
            section = "ast_context",
            chars = ast_context.len(),
            "Dropping section: prompt budget exceeded"
        );
        let dropped_chars = ast_context.len();
        ast_context.clear();
        estimated_size -= dropped_chars;
        if dropped_chars > 0 {
            budget_drops.push("ast_context".to_string());
        }
    }

    drop_full_content_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
        repo_path,
    );
    drop_patches_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
    );
}

/// Minimal chars kept for a budget-truncated patch. Guarantees the diff header
/// plus at least one hunk slice survives truncation, so a patch is never
/// emptied by the final-tier truncation path.
const MIN_PER_PATCH_TRUNCATION_CHARS: usize = 200;

/// Marker appended to patches truncated on the final budget tier. Mirrors the
/// convention used by `prompts/mod.rs`.
const PATCH_TRUNCATED_MARKER: &str =
    "[APTU: patch truncated due to prompt budget -- do not speculate on missing content]";

/// Drops file patches in descending size order until under budget.
///
/// This is the final budget tier: instead of nulling patches (which would
/// trigger `EmptyReviewContext` on a non-empty PR), each patch that must be
/// reduced is truncated to its leading hunks within a per-patch char budget
/// (even split of the total budget across the PR's patch files), so partial
/// context always survives. The per-patch budget is the budget remaining
/// after non-patch consumption, split evenly across the PR's patch files, so
/// a fully truncated PR still fits the prompt. Whole-patch drops in the
/// non-final tiers
/// (call graph, AST context, dep enrichments, full content) are unchanged.
pub(crate) fn drop_patches_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
) {
    if *estimated_size <= max_prompt_chars {
        return;
    }

    let patch_files = files
        .iter()
        .filter(|f| f.patch.as_deref().is_some_and(|p| !p.is_empty()))
        .count();
    if patch_files == 0 {
        return;
    }
    // Per-patch budget comes from the budget REMAINING after non-patch
    // consumption (estimated_size already includes the patch bytes), split
    // evenly across the PR's patch files, so a fully truncated PR still
    // fits the prompt.
    let total_patch_chars: usize = files
        .iter()
        .map(|f| f.patch.as_ref().map_or(0, String::len))
        .sum();
    let non_patch_chars = (*estimated_size).saturating_sub(total_patch_chars);
    let patch_headroom = max_prompt_chars.saturating_sub(non_patch_chars);
    let per_patch_budget = (patch_headroom / patch_files).max(MIN_PER_PATCH_TRUNCATION_CHARS);

    let mut file_sizes: Vec<(usize, usize)> = files
        .iter()
        .enumerate()
        .map(|(idx, f)| (idx, f.patch.as_ref().map_or(0, String::len)))
        .collect();
    file_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

    for (file_idx, patch_size) in file_sizes {
        if *estimated_size <= max_prompt_chars {
            break;
        }
        if patch_size == 0 || patch_size <= per_patch_budget {
            // Nothing to reclaim from this patch; keep it intact.
            continue;
        }
        let patch = files[file_idx].patch.as_deref().unwrap_or_default();
        // Reserve room for the marker so the final string stays within the
        // per-patch budget (and strictly smaller than the original patch).
        let core_budget = per_patch_budget
            .saturating_sub(PATCH_TRUNCATED_MARKER.len() + 1)
            .max(1);
        let truncated = super::truncation::truncate_at_line_boundary(patch, core_budget);
        let truncated = format!("{truncated}\n{PATCH_TRUNCATED_MARKER}");
        let removed = patch_size.saturating_sub(truncated.len());
        tracing::warn!(
            file = %files[file_idx].filename,
            patch_chars = patch_size,
            truncated_chars = truncated.len(),
            "Truncating patch to leading hunks: prompt budget exceeded"
        );
        let filename = files[file_idx].filename.clone();
        files[file_idx].patch = Some(truncated);
        *estimated_size = estimated_size.saturating_sub(removed);
        budget_drops.push(format!("patch_truncated:{filename}"));
    }
}

/// Builds a compact signature-outline block for a single file, mirroring the
/// per-function signature and imports summary used by `ast_context.rs`.
///
/// Returns `None` when the `ast-context` feature is disabled, the file's
/// extension is unsupported, or analysis fails.
#[cfg(feature = "ast-context")]
pub(crate) fn build_file_outline(repo_path: &str, filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if language_for_extension(ext).is_none() {
        tracing::debug!("build_file_outline: unsupported extension for {filename}: {ext:?}");
        return None;
    }
    let full_path = std::path::Path::new(repo_path).join(filename);
    let path_str = full_path.to_string_lossy().into_owned();

    match analyze_file(&path_str, None) {
        Ok(analysis) => {
            let mut outline = format!("## {filename}\n");
            for func in &analysis.semantic.functions {
                let _ = writeln!(outline, "  fn {}", func.compact_signature());
            }
            if !analysis.semantic.imports.is_empty() {
                outline.push_str("  imports:");
                for imp in analysis.semantic.imports.iter().take(5) {
                    let _ = write!(outline, " {}", imp.module);
                }
                outline.push('\n');
            }
            Some(outline)
        }
        Err(e) => {
            tracing::debug!("build_file_outline: skipping {filename}: {e}");
            None
        }
    }
}

/// Stub used when the `ast-context` feature is disabled; always returns `None`
/// so callers fall through to the existing full-clear behavior.
#[cfg(not(feature = "ast-context"))]
pub(crate) fn build_file_outline(_repo_path: &str, _filename: &str) -> Option<String> {
    None
}

/// Drops file `full_content` in descending size order until under budget.
///
/// Before fully clearing a file's `full_content`, attempts to substitute a
/// compact signature outline (via `build_file_outline`) when `repo_path` is
/// available and the outline actually brings the prompt back under budget.
/// Falls back to the existing full-clear behavior otherwise.
///
/// The `outline_len < content_size` check is load-bearing, not cosmetic: it
/// guarantees the substitution never grows the prompt relative to the
/// full-clear fallback it replaces.
pub(crate) fn drop_full_content_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
    repo_path: Option<&str>,
) {
    if *estimated_size <= max_prompt_chars {
        return;
    }

    let mut full_content_sizes: Vec<(usize, usize)> = files
        .iter()
        .enumerate()
        .map(|(idx, f)| (idx, f.full_content.as_ref().map_or(0, String::len)))
        .collect();
    full_content_sizes.sort_by_key(|x| std::cmp::Reverse(x.1));

    for (file_idx, content_size) in full_content_sizes {
        if *estimated_size <= max_prompt_chars {
            break;
        }
        if content_size == 0 {
            continue;
        }

        let filename = files[file_idx].filename.clone();

        if let Some(repo) = repo_path
            && let Some(outline) = build_file_outline(repo, &filename)
        {
            let outline_len = outline.len();
            let candidate_size = *estimated_size - content_size + outline_len;
            if outline_len < content_size && candidate_size <= max_prompt_chars {
                tracing::warn!(
                    file = %filename,
                    action = "outline_substituted",
                    original_chars = content_size,
                    outline_chars = outline_len,
                    "Substituting full_content with outline: prompt budget exceeded"
                );
                files[file_idx].full_content = Some(outline);
                *estimated_size = candidate_size;
                budget_drops.push(format!("file_content_outline:{filename}"));
                continue;
            }
        }

        tracing::warn!(
            file = %filename,
            action = "full_content_cleared",
            content_chars = content_size,
            "Dropping full_content: prompt budget exceeded"
        );
        files[file_idx].full_content = None;
        *estimated_size -= content_size;
        budget_drops.push(format!("file_content:{filename}"));
    }
}

/// Estimates the total character size of a PR review prompt.
///
/// Sums title, body, file metadata, patches, `full_content`,
/// `ast_context`, `call_graph`, `system_overhead` (system prompt plus repository
/// instructions), and overhead.
#[must_use]
pub(crate) fn estimate_pr_size(
    pr: &PrDetails,
    ast_context: &str,
    call_graph: &str,
    system_overhead: usize,
) -> usize {
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

    // Context
    size += ast_context.len();

    // Call graph
    size += call_graph.len();

    // Overhead
    size += PROMPT_OVERHEAD_CHARS;

    // System prompt + repository instructions
    size += system_overhead;

    size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_budget_caps_at_context_window() {
        // 10k tokens * 3.5 = 35k chars < 120k max_prompt_chars
        assert_eq!(effective_budget(120_000, Some(10_000), 0), 35_000);
    }

    #[test]
    fn test_effective_budget_subtracts_max_tokens() {
        // (10k window - 4k max_tokens) * 3.5 = 21k chars < 120k max_prompt_chars
        assert_eq!(effective_budget(120_000, Some(10_000), 4_000), 21_000);
    }

    #[test]
    fn test_effective_budget_none_is_unbounded() {
        assert_eq!(effective_budget(120_000, None, 4_000), 120_000);
    }

    #[test]
    fn test_effective_budget_overflow_saturates() {
        // window * 3.5 must not overflow; huge windows just leave the cap
        // at max_prompt_chars
        assert_eq!(effective_budget(120_000, Some(u32::MAX), 0), 120_000);
    }

    #[test]
    fn test_effective_budget_max_tokens_saturates_to_floor() {
        // max_tokens consuming the whole window (and beyond) must not reduce
        // the cap to zero; it saturates to MIN_EFFECTIVE_BUDGET_CHARS.
        assert_eq!(
            effective_budget(120_000, Some(4_000), 4_000),
            MIN_EFFECTIVE_BUDGET_CHARS
        );
        assert_eq!(
            effective_budget(120_000, Some(4_000), 10_000),
            MIN_EFFECTIVE_BUDGET_CHARS
        );
    }

    #[test]
    fn test_estimate_pr_size_includes_system_overhead() {
        let pr = PrDetails {
            title: "t".into(),
            body: "b".into(),
            head_branch: "h".into(),
            base_branch: "m".into(),
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
            url: String::new(),
            files: Vec::new(),
            labels: Vec::new(),
            head_sha: String::new(),
            review_comments: Vec::new(),
            instructions: None,
            prior_findings: None,
        };
        let without = estimate_pr_size(&pr, "", "", 0);
        let with = estimate_pr_size(&pr, "", "", 5_000);
        assert_eq!(with, without + 5_000);
    }
}
