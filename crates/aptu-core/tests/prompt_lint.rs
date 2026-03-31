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

/// Reconstructs a system prompt in the same way provider.rs does, for size testing.
fn triage_system_prompt() -> String {
    format!(
        "You are a senior OSS maintainer. Your mission is to produce structured triage output that helps maintainers prioritize and route incoming issues.\n\n{TOOLING_CONTEXT}\n\nYour response MUST be valid JSON with this exact schema:\n{TRIAGE_SCHEMA}\n\n{TRIAGE_GUIDELINES}"
    )
}

fn create_system_prompt() -> String {
    format!(
        "You are a senior developer advocate. Your mission is to produce a well-structured, professional GitHub issue from raw user input.\n\n{TOOLING_CONTEXT}\n\nYour response MUST be valid JSON with this exact schema:\n{CREATE_SCHEMA}\n\n{CREATE_GUIDELINES}"
    )
}

fn pr_review_system_prompt() -> String {
    format!(
        "You are a senior software engineer. Your mission is to produce structured, actionable review feedback on a pull request.\n\n{TOOLING_CONTEXT}\n\nYour response MUST be valid JSON with this exact schema:\n{PR_REVIEW_SCHEMA}\n\n{PR_REVIEW_GUIDELINES}"
    )
}

fn pr_label_system_prompt() -> String {
    format!(
        "You are a senior open-source maintainer. Your mission is to suggest the most relevant labels for a pull request based on its content.\n\n{TOOLING_CONTEXT}\n\nYour response MUST be valid JSON with this exact schema:\n{PR_LABEL_SCHEMA}\n\n{PR_LABEL_GUIDELINES}"
    )
}

fn release_notes_system_prompt() -> String {
    format!(
        "You are a senior release manager. Your mission is to produce clear, structured release notes.\n\nYour response MUST be valid JSON with this exact schema:\n{RELEASE_NOTES_SCHEMA}\n\n{RELEASE_NOTES_GUIDELINES}"
    )
}

#[test]
fn all_embedded_prompts_non_empty() {
    assert!(!TRIAGE_SCHEMA.is_empty(), "triage_schema.json is empty");
    assert!(
        !TRIAGE_GUIDELINES.is_empty(),
        "triage_guidelines.md is empty"
    );
    assert!(!CREATE_SCHEMA.is_empty(), "create_schema.json is empty");
    assert!(
        !CREATE_GUIDELINES.is_empty(),
        "create_guidelines.md is empty"
    );
    assert!(
        !PR_REVIEW_SCHEMA.is_empty(),
        "pr_review_schema.json is empty"
    );
    assert!(
        !PR_REVIEW_GUIDELINES.is_empty(),
        "pr_review_guidelines.md is empty"
    );
    assert!(!PR_LABEL_SCHEMA.is_empty(), "pr_label_schema.json is empty");
    assert!(
        !PR_LABEL_GUIDELINES.is_empty(),
        "pr_label_guidelines.md is empty"
    );
    assert!(
        !RELEASE_NOTES_SCHEMA.is_empty(),
        "release_notes_schema.json is empty"
    );
    assert!(
        !RELEASE_NOTES_GUIDELINES.is_empty(),
        "release_notes_guidelines.md is empty"
    );
    assert!(!TOOLING_CONTEXT.is_empty(), "tooling_context.md is empty");
}

#[test]
fn all_embedded_prompts_within_max_size() {
    const MAX: usize = 6000;
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
        assert!(
            prompt.len() <= MAX,
            "prompt '{name}' exceeds {MAX} chars: {} chars",
            prompt.len()
        );
    }
}

#[test]
fn system_prompts_have_persona_directive() {
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
        assert!(
            prompt.contains("You are a senior") || prompt.contains("You are a"),
            "prompt '{name}' missing persona directive"
        );
    }
}

#[test]
fn system_prompts_have_cot_directive() {
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
        assert!(
            prompt.contains("Reason through each step"),
            "prompt '{name}' missing CoT directive"
        );
    }
}

#[test]
fn system_prompts_have_examples_section() {
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
        assert!(
            prompt.contains("## Examples"),
            "prompt '{name}' missing ## Examples section"
        );
    }
}

#[test]
fn system_prompts_have_json_reminder_bookend() {
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
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
    let prompts = [
        ("triage", triage_system_prompt()),
        ("create", create_system_prompt()),
        ("pr_review", pr_review_system_prompt()),
        ("pr_label", pr_label_system_prompt()),
        ("release_notes", release_notes_system_prompt()),
    ];
    for (name, prompt) in &prompts {
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
    assert!(!TOOLING_CONTEXT.is_empty(), "tooling_context is empty");
}
