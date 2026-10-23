// SPDX-License-Identifier: Apache-2.0

//! Prompt size estimation and budget-driven enrichment drops.

use crate::ai::types::PrDetails;

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

/// Applies budget drop order: `full_content` -> patches.
/// Enforces the prompt budget by dropping enrichment sections in priority order.
///
/// When the assembled prompt exceeds `max_prompt_chars`, sections are cleared in
/// the following order (lowest-priority dropped first):
///
/// 1. file `full_content` -- dropped largest-first
/// 2. file patches -- dropped largest-first as last resort, so the diff itself
///    (the highest-value context for a review) is preserved as long as possible
///
/// Each drop is logged at `WARN` level with the section name and character count.
/// The function never returns an error; sections that cannot fit are silently cleared.
pub(crate) fn apply_budget_drops(
    pr: &mut PrDetails,
    max_prompt_chars: usize,
    system_overhead: usize,
    budget_drops: &mut Vec<String>,
) {
    let mut estimated_size = estimate_pr_size(pr, system_overhead);

    drop_full_content_by_size(
        &mut pr.files,
        &mut estimated_size,
        max_prompt_chars,
        budget_drops,
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
/// non-final tiers (dep enrichments, full content) are unchanged.
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

/// Drops file `full_content` in descending size order until under budget.
pub(crate) fn drop_full_content_by_size(
    files: &mut [crate::ai::types::PrFile],
    estimated_size: &mut usize,
    max_prompt_chars: usize,
    budget_drops: &mut Vec<String>,
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
/// `system_overhead` (system prompt plus repository instructions), and overhead.
#[must_use]
pub(crate) fn estimate_pr_size(pr: &PrDetails, system_overhead: usize) -> usize {
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
        let without = estimate_pr_size(&pr, 0);
        let with = estimate_pr_size(&pr, 5_000);
        assert_eq!(with, without + 5_000);
    }
}
