// SPDX-License-Identifier: Apache-2.0

//! Prompt quality invariant tests.
//!
//! Loads every embedded prompt fragment and asserts structural and size invariants
//! using the same builder functions that `provider.rs` uses at runtime. Tests
//! therefore validate the exact strings the AI receives, not a copy.

use aptu_core::ai::prompts::{
    TOOLING_CONTEXT, build_create_system_prompt, build_pr_label_system_prompt,
    build_pr_review_system_prompt, build_release_notes_system_prompt, build_triage_system_prompt,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns all system prompts as `(name, content)` pairs built the same way
/// `provider.rs` does at runtime. Tests iterate this list once rather than
/// repeating assertions per prompt.
fn all_system_prompts() -> Vec<(&'static str, String)> {
    vec![
        ("triage", build_triage_system_prompt(TOOLING_CONTEXT)),
        ("create", build_create_system_prompt(TOOLING_CONTEXT)),
        ("pr_review", build_pr_review_system_prompt(TOOLING_CONTEXT)),
        ("pr_label", build_pr_label_system_prompt(TOOLING_CONTEXT)),
        (
            "release_notes",
            build_release_notes_system_prompt(TOOLING_CONTEXT),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn all_embedded_prompts_non_empty() {
    for (name, prompt) in all_system_prompts() {
        assert!(!prompt.is_empty(), "prompt '{name}' is empty");
    }
    assert!(!TOOLING_CONTEXT.is_empty(), "tooling_context.md is empty");
}

#[test]
fn all_embedded_prompts_within_max_size() {
    const MAX: usize = 4000;
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
