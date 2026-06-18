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
