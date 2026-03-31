// SPDX-License-Identifier: Apache-2.0

//! Prompt quality invariant tests.
//!
//! Loads every embedded prompt file and asserts structural and size invariants.
//! These tests guard against prompt regressions introduced by future edits.

const TRIAGE_SCHEMA: &str = include_str!("../src/ai/prompts/triage_schema.json");
const TRIAGE_GUIDELINES: &str = include_str!("../src/ai/prompts/triage_guidelines.md");
const CREATE_SCHEMA: &str = include_str!("../src/ai/prompts/create_schema.json");
const CREATE_GUIDELINES: &str = include_str!("../src/ai/prompts/create_guidelines.md");
const PR_REVIEW_SCHEMA: &str = include_str!("../src/ai/prompts/pr_review_schema.json");
const PR_REVIEW_GUIDELINES: &str = include_str!("../src/ai/prompts/pr_review_guidelines.md");
const PR_LABEL_SCHEMA: &str = include_str!("../src/ai/prompts/pr_label_schema.json");
const PR_LABEL_GUIDELINES: &str = include_str!("../src/ai/prompts/pr_label_guidelines.md");
const RELEASE_NOTES_SCHEMA: &str = include_str!("../src/ai/prompts/release_notes_schema.json");
const RELEASE_NOTES_GUIDELINES: &str =
    include_str!("../src/ai/prompts/release_notes_guidelines.md");
const TOOLING_CONTEXT: &str = include_str!("../src/ai/prompts/tooling_context.md");

/// Returns all system prompts as `(name, content)` pairs, reconstructed the same
/// way `provider.rs` does at runtime. Tests iterate this list rather than repeating
/// the same assertion for each prompt individually.
fn all_system_prompts() -> Vec<(&'static str, String)> {
    vec![
        (
            "triage",
            format!(
                "You are a senior OSS maintainer. Your mission is to produce structured triage \
                 output that helps maintainers prioritize and route incoming issues.\n\n\
                 {TOOLING_CONTEXT}\n\n\
                 Your response MUST be valid JSON with this exact schema:\n{TRIAGE_SCHEMA}\n\n\
                 {TRIAGE_GUIDELINES}"
            ),
        ),
        (
            "create",
            format!(
                "You are a senior developer advocate. Your mission is to produce a well-structured, \
                 professional GitHub issue from raw user input.\n\n\
                 {TOOLING_CONTEXT}\n\n\
                 Your response MUST be valid JSON with this exact schema:\n{CREATE_SCHEMA}\n\n\
                 {CREATE_GUIDELINES}"
            ),
        ),
        (
            "pr_review",
            format!(
                "You are a senior software engineer. Your mission is to produce structured, \
                 actionable review feedback on a pull request.\n\n\
                 {TOOLING_CONTEXT}\n\n\
                 Your response MUST be valid JSON with this exact schema:\n{PR_REVIEW_SCHEMA}\n\n\
                 {PR_REVIEW_GUIDELINES}"
            ),
        ),
        (
            "pr_label",
            format!(
                "You are a senior open-source maintainer. Your mission is to suggest the most \
                 relevant labels for a pull request based on its content.\n\n\
                 {TOOLING_CONTEXT}\n\n\
                 Your response MUST be valid JSON with this exact schema:\n{PR_LABEL_SCHEMA}\n\n\
                 {PR_LABEL_GUIDELINES}"
            ),
        ),
        (
            "release_notes",
            format!(
                "You are a senior release manager. Your mission is to produce clear, structured \
                 release notes.\n\n\
                 Your response MUST be valid JSON with this exact schema:\n{RELEASE_NOTES_SCHEMA}\n\n\
                 {RELEASE_NOTES_GUIDELINES}"
            ),
        ),
    ]
}

#[test]
fn all_embedded_prompts_non_empty() {
    let fragments: &[(&str, &str)] = &[
        ("triage_schema.json", TRIAGE_SCHEMA),
        ("triage_guidelines.md", TRIAGE_GUIDELINES),
        ("create_schema.json", CREATE_SCHEMA),
        ("create_guidelines.md", CREATE_GUIDELINES),
        ("pr_review_schema.json", PR_REVIEW_SCHEMA),
        ("pr_review_guidelines.md", PR_REVIEW_GUIDELINES),
        ("pr_label_schema.json", PR_LABEL_SCHEMA),
        ("pr_label_guidelines.md", PR_LABEL_GUIDELINES),
        ("release_notes_schema.json", RELEASE_NOTES_SCHEMA),
        ("release_notes_guidelines.md", RELEASE_NOTES_GUIDELINES),
        ("tooling_context.md", TOOLING_CONTEXT),
    ];
    for (name, content) in fragments {
        assert!(!content.is_empty(), "{name} is empty");
    }
}

#[test]
fn all_embedded_prompts_within_max_size() {
    const MAX: usize = 6000;
    for (name, prompt) in all_system_prompts() {
        assert!(
            prompt.len() <= MAX,
            "prompt '{name}' exceeds {MAX} chars: {} chars",
            prompt.len()
        );
    }
}

#[test]
fn system_prompts_have_persona_directive() {
    for (name, prompt) in all_system_prompts() {
        assert!(
            prompt.contains("You are a"),
            "prompt '{name}' missing persona directive"
        );
    }
}

#[test]
fn system_prompts_have_cot_directive() {
    for (name, prompt) in all_system_prompts() {
        assert!(
            prompt.contains("Reason through each step"),
            "prompt '{name}' missing CoT directive"
        );
    }
}

#[test]
fn system_prompts_have_examples_section() {
    for (name, prompt) in all_system_prompts() {
        assert!(
            prompt.contains("## Examples"),
            "prompt '{name}' missing ## Examples section"
        );
    }
}

#[test]
fn system_prompts_have_json_reminder_bookend() {
    for (name, prompt) in all_system_prompts() {
        let tail = &prompt[prompt.len().saturating_sub(300)..];
        assert!(
            tail.contains("valid JSON") || tail.contains("schema"),
            "prompt '{name}' missing JSON reminder in last 300 chars"
        );
    }
}

#[test]
fn system_prompts_meet_minimum_size() {
    const MIN: usize = 200;
    for (name, prompt) in all_system_prompts() {
        assert!(
            prompt.len() >= MIN,
            "prompt '{name}' is too short: {} chars (min {MIN})",
            prompt.len()
        );
    }
}

#[test]
fn tooling_context_contains_required_tools() {
    assert!(
        TOOLING_CONTEXT.contains("ruff"),
        "tooling_context missing 'ruff'"
    );
    assert!(
        TOOLING_CONTEXT.contains("biome"),
        "tooling_context missing 'biome'"
    );
}
