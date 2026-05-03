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
/// - `max_chars_per_file`: 4,000 chars per file keeps individual file snippets readable
///   without dominating the prompt budget.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ReviewConfig {
    /// Maximum total prompt character budget (default: `120_000`).
    pub max_prompt_chars: usize,
    /// Maximum number of files to fetch full content for (default: 10).
    pub max_full_content_files: usize,
    /// Maximum characters per file's full content (default: `4_000`).
    pub max_chars_per_file: usize,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            max_prompt_chars: 120_000, // Conservative budget for LLM context windows with overhead
            max_full_content_files: 10, // Cap GitHub Contents API calls to limit latency and rate limits
            max_chars_per_file: 4_000, // Keep individual file snippets readable without overwhelming prompt
        }
    }
}
