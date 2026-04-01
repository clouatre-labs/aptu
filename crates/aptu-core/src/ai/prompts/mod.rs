// SPDX-License-Identifier: Apache-2.0

//! Compile-time prompt fragments and system-prompt builder functions.
//!
//! Schema (JSON) and guidelines (Markdown) files are embedded at compile time
//! via [`include_str!`]. Paths are relative to this file
//! (`crates/aptu-core/src/ai/prompts/`); if this module is moved the compiler
//! will error at the `include_str!` call sites, making stale paths impossible
//! to miss.
//!
//! Both `provider.rs` (runtime) and `tests/prompt_lint.rs` (tests) import the
//! public builder functions here to guarantee they test the same construction
//! logic.

// ---------------------------------------------------------------------------
// Embedded fragments
// ---------------------------------------------------------------------------

/// JSON schema for issue triage responses.
pub const TRIAGE_SCHEMA: &str = include_str!("triage_schema.json");
/// Guidelines and examples for issue triage system prompts.
pub const TRIAGE_GUIDELINES: &str = include_str!("triage_guidelines.md");
/// JSON schema for issue creation responses.
pub const CREATE_SCHEMA: &str = include_str!("create_schema.json");
/// Guidelines and examples for issue creation system prompts.
pub const CREATE_GUIDELINES: &str = include_str!("create_guidelines.md");
/// JSON schema for PR review responses.
pub const PR_REVIEW_SCHEMA: &str = include_str!("pr_review_schema.json");
/// Guidelines and examples for PR review system prompts.
pub const PR_REVIEW_GUIDELINES: &str = include_str!("pr_review_guidelines.md");
/// JSON schema for PR label suggestion responses.
pub const PR_LABEL_SCHEMA: &str = include_str!("pr_label_schema.json");
/// Guidelines and examples for PR label suggestion system prompts.
pub const PR_LABEL_GUIDELINES: &str = include_str!("pr_label_guidelines.md");
/// JSON schema for release notes responses.
pub const RELEASE_NOTES_SCHEMA: &str = include_str!("release_notes_schema.json");
/// Guidelines and examples for release notes system prompts.
pub const RELEASE_NOTES_GUIDELINES: &str = include_str!("release_notes_guidelines.md");
/// Best-practices context injected into all system prompts (tooling recommendations).
pub const TOOLING_CONTEXT: &str = include_str!("tooling_context.md");

// ---------------------------------------------------------------------------
// Public builder functions (shared between provider.rs and prompt_lint tests)
// ---------------------------------------------------------------------------

/// Builds the system prompt for issue triage.
#[must_use]
pub fn build_triage_system_prompt(context: &str) -> String {
    format!(
        "You are a senior OSS maintainer. Your mission is to produce structured triage output \
         that helps maintainers prioritize and route incoming issues.\n\n\
         {context}\n\n\
         {TRIAGE_GUIDELINES}"
    )
}

/// Builds the system prompt for issue creation/formatting.
#[must_use]
pub fn build_create_system_prompt(context: &str) -> String {
    format!(
        "You are a senior developer advocate. Your mission is to produce a well-structured, \
         professional GitHub issue from raw user input.\n\n\
         {context}\n\n\
         {CREATE_GUIDELINES}"
    )
}

/// Builds the system prompt for PR review.
#[must_use]
pub fn build_pr_review_system_prompt(context: &str) -> String {
    format!(
        "You are a senior software engineer. Your mission is to produce structured, actionable \
         review feedback on a pull request.\n\n\
         {context}\n\n\
         {PR_REVIEW_GUIDELINES}"
    )
}

/// Builds the system prompt for PR label suggestion.
#[must_use]
pub fn build_pr_label_system_prompt(context: &str) -> String {
    format!(
        "You are a senior open-source maintainer. Your mission is to suggest the most relevant \
         labels for a pull request based on its content.\n\n\
         {context}\n\n\
         {PR_LABEL_GUIDELINES}"
    )
}

/// Builds the system prompt for release notes generation.
#[must_use]
pub fn build_release_notes_system_prompt(context: &str) -> String {
    format!(
        "You are a senior release manager. Your mission is to produce clear, structured release \
         notes.\n\n\
         {context}\n\n\
         {RELEASE_NOTES_GUIDELINES}"
    )
}
