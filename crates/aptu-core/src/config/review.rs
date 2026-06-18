// SPDX-License-Identifier: Apache-2.0

//! PR review prompt configuration.

use serde::{Deserialize, Serialize};

/// PR review prompt configuration.
///
/// Controls prompt token budgets and GitHub API constraints for PR reviews:
///
/// - `max_prompt_chars`: 120,000 chars is a conservative budget below common LLM context
///   window limits (e.g., 128k token models), accounting for system prompt and response overhead.
/// - `max_full_content_files`: 10 files caps GitHub Contents API calls per review to limit
///   latency and rate limit usage.
/// - `max_chars_per_file`: 16,000 chars per file gives adequate context for most files
///   without dominating the prompt budget; budget drop logic trims below 120k if needed.
/// - `max_instructions_chars`: 1,500 chars caps repository instructions to prevent prompt bloat.
/// - `max_diff_chars`: 200,000 chars caps the total diff content across all files in the prompt.
/// - `max_patch_chars_per_file`: 10,000 chars caps each individual file patch before dropping it entirely.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ReviewConfig {
    /// Maximum total prompt character budget (default: `120_000`).
    pub max_prompt_chars: usize,
    /// Maximum number of files to fetch full content for (default: 10).
    pub max_full_content_files: usize,
    /// Maximum characters per file's full content (default: `16_000`).
    pub max_chars_per_file: usize,
    /// Maximum total diff characters across all files in the prompt (default: `200_000`).
    pub max_diff_chars: usize,
    /// Maximum characters per individual file patch before the patch is dropped entirely (default: `10_000`).
    pub max_patch_chars_per_file: usize,
    /// Maximum characters for repository instructions (default: `1_500`).
    #[serde(default = "default_max_instructions_chars")]
    pub max_instructions_chars: usize,
    /// Optional path to repository instructions file (overrides default AGENTS.md and .github/instructions/pr-review.md).
    #[serde(default)]
    pub instructions_file: Option<String>,
    /// Minimum remaining prompt budget to auto-enable call graph (default: `20_000`).
    ///
    /// The call graph is built only when:
    /// `budget_remaining = max_prompt_chars - estimated_size (call_graph excluded)`
    /// and `budget_remaining > min_budget_for_call_graph`.
    ///
    /// A value >= `max_prompt_chars` means call graph is never auto-enabled.
    /// A value > `max_prompt_chars / 2` means call graph is only built for
    /// the largest diffs — consider lowering the threshold.
    #[serde(default = "default_min_budget_for_call_graph")]
    pub min_budget_for_call_graph: usize,
    /// Maximum characters for dependency release notes (default: `2_000`).
    #[serde(default = "default_max_dep_release_chars")]
    pub max_dep_release_chars: usize,
    /// Maximum number of dependency packages to enrich (default: 3).
    #[serde(default = "default_max_dep_packages")]
    pub max_dep_packages: usize,
}

fn default_max_instructions_chars() -> usize {
    1_500
}

fn default_min_budget_for_call_graph() -> usize {
    20_000
}

fn default_max_dep_release_chars() -> usize {
    2_000
}

fn default_max_dep_packages() -> usize {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_consistency_ok() {
        let config = ReviewConfig::default();
        let warnings = config.validate_consistency();
        assert!(
            warnings.is_empty(),
            "default config should produce no warnings: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validate_consistency_threshold_equals_max() {
        let config = ReviewConfig {
            min_budget_for_call_graph: 120_000,
            max_prompt_chars: 120_000,
            ..ReviewConfig::default()
        };
        let warnings = config.validate_consistency();
        assert_eq!(warnings.len(), 1, "should produce exactly 1 warning");
        assert!(
            warnings[0].contains("call_graph will never be built"),
            "warning should indicate call_graph is never built: {}",
            warnings[0]
        );
    }

    #[test]
    fn test_validate_consistency_threshold_over_half() {
        let config = ReviewConfig {
            min_budget_for_call_graph: 80_000,
            max_prompt_chars: 120_000,
            ..ReviewConfig::default()
        };
        let warnings = config.validate_consistency();
        assert_eq!(warnings.len(), 1, "should produce exactly 1 warning");
        assert!(
            warnings[0].contains("only be built for the largest diffs"),
            "warning should indicate call_graph rarely enables: {}",
            warnings[0]
        );
    }
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            max_prompt_chars: 120_000,
            max_full_content_files: 10,
            max_chars_per_file: 16_000,
            max_diff_chars: 200_000,
            max_patch_chars_per_file: 10_000,
            max_instructions_chars: 1_500,
            instructions_file: None,
            min_budget_for_call_graph: 20_000,
            max_dep_release_chars: 2_000,
            max_dep_packages: 3,
        }
    }
}

impl ReviewConfig {
    /// Validate internal consistency of review configuration.
    ///
    /// Returns a list of warning strings for any misconfigured values.
    /// The caller should emit these warnings via `tracing::warn!` or similar.
    #[must_use]
    pub fn validate_consistency(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Warning 1: min_budget_for_call_graph >= max_prompt_chars means
        // call_graph is never auto-enabled (budget_remaining is always <= max_prompt_chars
        // and must be > min_budget_for_call_graph).
        if self.min_budget_for_call_graph >= self.max_prompt_chars {
            warnings.push(format!(
                "min_budget_for_call_graph ({}) >= max_prompt_chars ({}): call_graph will never be built; call_graph is enabled only when budget_remaining > min_budget_for_call_graph",
                self.min_budget_for_call_graph, self.max_prompt_chars
            ));
        }
        // Warning 2: min_budget_for_call_graph > max_prompt_chars / 2 but < max_prompt_chars
        // means call_graph will only be built for the largest diffs.
        else if self.min_budget_for_call_graph > self.max_prompt_chars / 2 {
            warnings.push(format!(
                "min_budget_for_call_graph ({}) exceeds half of max_prompt_chars ({}): call_graph will only be built for the largest diffs; consider lowering the threshold",
                self.min_budget_for_call_graph, self.max_prompt_chars
            ));
        }

        warnings
    }
}
