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
use aptu_core::ai::provider::{AiProvider, MAX_FULL_CONTENT_CHARS};
use aptu_core::ai::types::{IssueDetails, PrDetails, PrFile, PrSummary};

// ---------------------------------------------------------------------------
// Minimal provider stub for user-prompt builder access
// ---------------------------------------------------------------------------

struct StubProvider;

impl AiProvider for StubProvider {
    fn name(&self) -> &str {
        "stub"
    }
    fn api_url(&self) -> &str {
        "https://stub.example.com"
    }
    fn api_key_env(&self) -> &str {
        "STUB_API_KEY"
    }
    fn http_client(&self) -> &reqwest::Client {
        unimplemented!()
    }
    fn api_key(&self) -> &secrecy::SecretString {
        unimplemented!()
    }
    fn model(&self) -> &str {
        "stub-model"
    }
    fn max_tokens(&self) -> u32 {
        2048
    }
    fn temperature(&self) -> f32 {
        0.3
    }
}

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
    // Ceiling empirically grounded: Goldberg et al. (arXiv:2402.14848) show reasoning
    // degradation around 3,000 tokens; 5,000 chars (~1,250 tokens at ~4 chars/token)
    // provides headroom below that threshold while accommodating all current guidelines
    // without content removal. Triage is the largest prompt at ~4,759 chars.
    const MAX: usize = 5000;
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

#[test]
fn all_user_prompts_contain_schema() {
    // triage user prompt
    let issue = IssueDetails::builder()
        .owner("test".to_string())
        .repo("repo".to_string())
        .number(1)
        .title("Test issue".to_string())
        .body("Issue body".to_string())
        .labels(vec![])
        .comments(vec![])
        .url("https://github.com/test/repo/issues/1".to_string())
        .build();
    let triage_user = StubProvider::build_user_prompt(&issue);
    assert!(
        triage_user.contains("summary") && triage_user.contains("suggested_labels"),
        "triage user prompt missing schema fields"
    );

    // create user prompt
    let create_user = StubProvider::build_create_user_prompt("My title", "My body", "test/repo");
    assert!(
        create_user.contains("formatted_title") && create_user.contains("formatted_body"),
        "create user prompt missing schema fields"
    );

    // pr_review user prompt
    let pr = PrDetails {
        owner: "test".to_string(),
        repo: "repo".to_string(),
        number: 1,
        title: "Test PR".to_string(),
        body: "PR body".to_string(),
        head_branch: "feat".to_string(),
        base_branch: "main".to_string(),
        url: "https://github.com/test/repo/pull/1".to_string(),
        files: vec![PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            additions: 5,
            deletions: 2,
            patch: None,
            full_content: None,
        }],
        labels: vec![],
        head_sha: String::new(),
    };
    let pr_review_user = StubProvider::build_pr_review_user_prompt(&pr, "", "");
    assert!(
        pr_review_user.contains("verdict") && pr_review_user.contains("summary"),
        "pr_review user prompt missing schema fields"
    );

    // pr_label user prompt
    let pr_label_user = StubProvider::build_pr_label_user_prompt(
        "feat: add thing",
        "body",
        &["src/lib.rs".to_string()],
    );
    assert!(
        pr_label_user.contains("suggested_labels"),
        "pr_label user prompt missing schema fields"
    );

    // release_notes user prompt
    let prs = vec![PrSummary {
        number: 1,
        title: "feat: add thing".to_string(),
        author: "alice".to_string(),
        body: "First line of PR body".to_string(),
        merged_at: None,
    }];
    let release_user = StubProvider::build_release_notes_prompt(&prs, "v1.0.0");
    assert!(
        release_user.contains("theme") && release_user.contains("highlights"),
        "release_notes user prompt missing schema fields"
    );
}

#[cfg(test)]
mod fetch_file_contents_tests {
    use super::*;

    #[test]
    fn test_file_content_injected_into_prompt() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "PR body".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "src/lib.rs".to_string(),
                status: "modified".to_string(),
                additions: 5,
                deletions: 2,
                patch: Some("@@ -1,3 +1,4 @@\n+// new line".to_string()),
                full_content: Some("fn hello() {}".to_string()),
            }],
            labels: vec![],
            head_sha: String::new(),
        };
        let prompt = StubProvider::build_pr_review_user_prompt(&pr, "", "");
        assert!(
            prompt.contains("<file_content path=\"src/lib.rs\">"),
            "Prompt should contain file_content block"
        );
        assert!(
            prompt.contains("fn hello() {}"),
            "Prompt should contain full file content"
        );
    }

    #[test]
    fn test_file_content_truncated_at_prompt_assembly() {
        // Arrange: full_content longer than MAX_FULL_CONTENT_CHARS
        let long_content = "x".repeat(MAX_FULL_CONTENT_CHARS + 1000);
        assert!(long_content.len() > MAX_FULL_CONTENT_CHARS);
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "PR body".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "huge.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 0,
                patch: Some("@@ -1,1 +1,1 @@\n+x".to_string()),
                full_content: Some(long_content.clone()),
            }],
            labels: vec![],
            head_sha: String::new(),
        };

        // Act
        let prompt = StubProvider::build_pr_review_user_prompt(&pr, "", "");

        // Assert: block present but content capped at MAX_FULL_CONTENT_CHARS
        assert!(
            prompt.contains("<file_content path=\"huge.rs\">"),
            "Prompt should contain file_content block"
        );
        let block_start = prompt
            .find("<file_content path=\"huge.rs\">\n")
            .expect("file_content block start");
        let content_start = block_start + "<file_content path=\"huge.rs\">\n".len();
        let content_end = prompt[content_start..]
            .find("\n</file_content>")
            .expect("file_content block end");
        let included_content = &prompt[content_start..content_start + content_end];
        assert_eq!(
            included_content.len(),
            MAX_FULL_CONTENT_CHARS,
            "file_content in prompt must be capped at MAX_FULL_CONTENT_CHARS"
        );
    }

    #[test]
    fn test_build_pr_review_prompt_includes_call_graph_when_present() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "PR body".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "src/lib.rs".to_string(),
                status: "modified".to_string(),
                additions: 5,
                deletions: 2,
                patch: Some("@@ -1,3 +1,4 @@\n+// new line".to_string()),
                full_content: None,
            }],
            labels: vec![],
            head_sha: String::new(),
        };
        // Just verify that the prompt builder itself includes call_graph when provided
        let large_call_graph = "<call_graph>".to_string() + &"x".repeat(1000) + "</call_graph>";
        let prompt = StubProvider::build_pr_review_user_prompt(&pr, "", &large_call_graph);
        // The prompt builder includes call_graph as-is; budget enforcement is done in review_pr
        assert!(
            prompt.contains(&large_call_graph),
            "Call graph should be in prompt at builder level"
        );
    }
}
