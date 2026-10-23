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
/// - `max_chars_per_file`: 32,000 chars per file gives adequate context for most files
///   without dominating the prompt budget; budget drop logic trims below 120k if needed.
/// - `max_instructions_chars`: 1,500 chars caps repository instructions to prevent prompt bloat.
/// - `max_diff_chars`: 120,000 chars caps the total diff content across all files in the prompt
///   (aligned with `max_prompt_chars` so the diff budget never exceeds the prompt budget).
/// - `max_patch_chars_per_file`: 25,000 chars caps each individual file patch before dropping it entirely.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ReviewConfig {
    /// Maximum total prompt character budget (default: `120_000`).
    pub max_prompt_chars: usize,
    /// Maximum number of files to fetch full content for (default: 10).
    pub max_full_content_files: usize,
    /// Maximum characters per file's full content (default: `32_000`).
    pub max_chars_per_file: usize,
    /// Maximum total diff characters across all files in the prompt (default: `120_000`).
    pub max_diff_chars: usize,
    /// Maximum characters per individual file patch before the patch is dropped entirely (default: `25_000`).
    pub max_patch_chars_per_file: usize,
    /// Maximum characters for repository instructions (default: `1_500`).
    #[serde(default = "default_max_instructions_chars")]
    pub max_instructions_chars: usize,
    /// Optional path to repository instructions file (overrides default AGENTS.md and .github/instructions/pr-review.md).
    #[serde(default)]
    pub instructions_file: Option<String>,
    /// Enable the AI findings verification pass (default: `true`).
    ///
    /// When `true`, one extra small-model call per review verifies each
    /// candidate finding against the diff; unverifiable or low-confidence
    /// findings are dropped before posting.
    #[serde(default = "default_true")]
    pub verify_enabled: bool,
    /// Minimum confidence (0.0-1.0) a finding must reach in the verification
    /// pass to be kept (default: `0.7`).
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    /// Suppress info-severity findings from the posted review (default: `true`).
    #[serde(default = "default_true")]
    pub suppress_info: bool,
}

fn default_true() -> bool {
    true
}

fn default_min_confidence() -> f32 {
    0.7
}

fn default_max_instructions_chars() -> usize {
    1_500
}

#[cfg(test)]
mod tests {
    #![allow(clippy::items_after_test_module)]
    use super::*;

    #[test]
    fn test_validate_consistency_ok() {
        let config = ReviewConfig::default();
        let warnings = config.validate_consistency();
        assert!(
            warnings.is_empty(),
            "default config should produce no warnings: {warnings:?}"
        );
    }

    #[test]
    fn test_validate_consistency_max_diff_over_prompt_warns_once() {
        let config = ReviewConfig {
            max_diff_chars: 200_000,
            max_prompt_chars: 120_000,
            ..ReviewConfig::default()
        };
        let warnings = config.validate_consistency();
        assert_eq!(warnings.len(), 1, "should produce exactly 1 warning");
        assert!(
            warnings[0].contains("max_diff_chars"),
            "warning should mention max_diff_chars: {}",
            warnings[0]
        );
    }

    #[test]
    fn test_default_max_patch_chars_per_file() {
        let config = ReviewConfig::default();
        assert_eq!(
            config.max_patch_chars_per_file, 25_000,
            "default max_patch_chars_per_file should be 25_000"
        );
    }

    #[test]
    fn test_review_verification_defaults() {
        let config = ReviewConfig::default();
        assert!(
            config.verify_enabled,
            "verify_enabled should default to true"
        );
        assert_eq!(
            config.min_confidence, 0.7,
            "default min_confidence should be 0.7"
        );
        assert!(config.suppress_info, "suppress_info should default to true");
    }
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            max_prompt_chars: 120_000,
            max_full_content_files: 10,
            max_chars_per_file: 32_000,
            max_diff_chars: 120_000,
            max_patch_chars_per_file: 25_000,
            max_instructions_chars: 1_500,
            instructions_file: None,
            verify_enabled: true,
            min_confidence: 0.7,
            suppress_info: true,
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

        // Warning 1: max_diff_chars > max_prompt_chars means the per-file diff
        // budgets can produce more diff content than the prompt can hold, so
        // budget drops would evict diff content silently.
        if self.max_diff_chars > self.max_prompt_chars {
            warnings.push(format!(
                "max_diff_chars ({}) exceeds max_prompt_chars ({}): diff content will be dropped by budget logic; lower max_diff_chars to at most max_prompt_chars",
                self.max_diff_chars, self.max_prompt_chars
            ));
        }

        warnings
    }
}
