// SPDX-License-Identifier: Apache-2.0

//! PR review: review a pull request using the AI provider.
//!
//! Provides `review_pr`, `estimate_pr_size`, and prompt builder helpers.

use anyhow::Result;
use tracing::{debug, instrument};

use super::http::send_and_parse;
use super::parse::provider_response_format;
use crate::ai::provider::AiProvider;
use crate::ai::types::{ChatCompletionRequest, ChatMessage, PrDetails, PrReviewResponse};
use crate::history::AiStats;

use crate::ai::prompts::build_pr_review_system_prompt;

/// Estimated overhead for XML tags, section headers, and schema preamble added by
/// `build_pr_review_user_prompt`. Used to ensure the prompt budget accounts for
/// non-content characters when estimating total prompt size.
const PROMPT_OVERHEAD_CHARS: usize = 1_000;

/// Estimates the initial size of a PR review prompt in characters.
///
/// Sums title, body, file metadata, patches, `full_content`, `dep_enrichments`,
/// `ast_context`, `call_graph`, and overhead.
#[must_use]
pub(super) fn estimate_pr_size(pr: &PrDetails, ast_context: &str, call_graph: &str) -> usize {
    pr.title.len()
        + pr.body.len()
        + pr.files
            .iter()
            .map(|f| f.patch.as_ref().map_or(0, String::len))
            .sum::<usize>()
        + pr.files
            .iter()
            .map(|f| f.full_content.as_ref().map_or(0, String::len))
            .sum::<usize>()
        + pr.dep_enrichments
            .iter()
            .map(|d| d.body.len() + d.package_name.len() + d.github_url.len())
            .sum::<usize>()
        + ast_context.len()
        + call_graph.len()
        + PROMPT_OVERHEAD_CHARS
}

/// Builds the system prompt for PR review.
#[must_use]
pub(super) fn build_pr_review_system_prompt_fn(custom_guidance: Option<&str>) -> String {
    let context = crate::ai::context::load_custom_guidance(custom_guidance);
    build_pr_review_system_prompt(&context)
}

/// Builds the user prompt for PR review.
///
/// All user-controlled fields (title, body, filename, status, patch) are sanitized via
/// [`sanitize_prompt_field`] before being written into the prompt to prevent prompt
/// injection via XML tag smuggling.
#[allow(clippy::too_many_lines)]
#[must_use]
pub(super) fn build_pr_review_user_prompt(
    ctx: &mut crate::ai::review_context::ReviewContext,
) -> String {
    crate::ai::prompts::build_pr_review_user_prompt(ctx)
}

/// Reviews a pull request using the provider's API.
///
/// Analyzes PR metadata and file diffs to provide structured review feedback.
///
/// # Arguments
///
/// * `ctx` - Review context including PR details
/// * `review_config` - Configuration for review prompts
///
/// # Concurrency
///
/// `ctx` is owned by each call; truncation counter mutations inside
/// `build_pr_review_user_prompt` are local to that invocation and are never
/// shared across concurrent calls.
///
/// # Errors
///
/// Returns an error if:
/// - API request fails (network, timeout, rate limit)
/// - Response cannot be parsed as valid JSON
#[instrument(skip(provider, ctx), fields(pr_number = ctx.pr.number, repo = %format!("{}/{}", ctx.pr.owner, ctx.pr.repo)))]
#[allow(unused_assignments)]
pub(super) async fn review_pr(
    provider: &(impl AiProvider + ?Sized),
    mut ctx: crate::ai::review_context::ReviewContext,
    review_config: &crate::config::ReviewConfig,
) -> Result<(PrReviewResponse, AiStats, Vec<String>)> {
    debug!(model = %provider.model(), "Calling {} API for PR review", provider.name());

    // Build request
    #[cfg(not(target_arch = "wasm32"))]
    let mut system_content = if let Some(override_prompt) =
        crate::ai::context::load_system_prompt_override("pr_review_system").await
    {
        override_prompt
    } else {
        build_pr_review_system_prompt_fn(provider.custom_guidance())
    };
    #[cfg(target_arch = "wasm32")]
    let mut system_content = build_pr_review_system_prompt_fn(provider.custom_guidance());

    // Prepend repository instructions if available
    if let Some(instructions) = &ctx.pr.instructions {
        // Escape XML delimiters to prevent tag injection
        let escaped_instructions = instructions
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        system_content = format!(
            "<repo_instructions>\n{escaped_instructions}\n</repo_instructions>\n\n{system_content}"
        );
    }

    // Assemble full prompt to measure actual size
    let assembled_prompt = crate::ai::prompts::build_pr_review_user_prompt(&mut ctx);
    let actual_prompt_chars = assembled_prompt.len();
    ctx.prompt_chars_final = actual_prompt_chars;

    tracing::info!(
        actual_prompt_chars,
        max_chars = review_config.max_prompt_chars,
        "PR review prompt assembled"
    );

    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Some(system_content),
            reasoning: None,
            cache_control: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: Some(assembled_prompt),
            reasoning: None,
            cache_control: None,
        },
    ];

    // Inject cache control on system message for Anthropic
    if provider.is_anthropic()
        && let Some(msg) = messages.first_mut()
    {
        msg.cache_control = Some(crate::ai::types::CacheControl::ephemeral());
    }

    let request = ChatCompletionRequest {
        model: provider.model().to_string(),
        messages,
        response_format: provider_response_format(provider),
        max_tokens: Some(provider.max_tokens()),
        temperature: Some(provider.temperature()),
    };

    // Send request and parse JSON with retry logic
    let (review, mut ai_stats, finish_reasons) =
        send_and_parse::<PrReviewResponse>(provider, &request).await?;

    ai_stats.prompt_chars = actual_prompt_chars;

    debug!(
        verdict = %review.verdict,
        input_tokens = ai_stats.input_tokens,
        output_tokens = ai_stats.output_tokens,
        duration_ms = ai_stats.duration_ms,
        prompt_chars = ai_stats.prompt_chars,
        "PR review complete with stats"
    );

    Ok((review, ai_stats, finish_reasons))
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;
    use crate::ai::provider::MAX_BODY_LENGTH;
    use crate::ai::review_context::ReviewContext;
    use crate::ai::types::{DepReleaseNote, PrDetails, PrFile};

    #[test]
    fn test_build_pr_review_user_prompt_respects_file_limit() {
        let mut files: Vec<PrFile> = (0..25)
            .map(|i| PrFile {
                filename: format!("file{i}.rs"),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("diff content".to_string()),
                patch_truncated: false,
                full_content: None,
            })
            .collect();
        files.push(PrFile {
            filename: "extra.rs".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: Some("extra content".to_string()),
            patch_truncated: false,
            full_content: None,
        });

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(prompt.contains("files omitted due to size limits"));
    }

    #[test]
    fn test_build_pr_review_user_prompt_respects_diff_size_limit() {
        let patch1 = "x".repeat(3_000);
        let patch2 = "y".repeat(3_000);

        let files = vec![
            PrFile {
                filename: "file1.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 50,
                patch: Some(patch1),
                patch_truncated: false,
                full_content: None,
            },
            PrFile {
                filename: "file2.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 50,
                patch: Some(patch2),
                patch_truncated: false,
                full_content: None,
            },
        ];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            max_diff_chars: 4_000,
            max_patch_chars_per_file: 10_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        // file1's 3k patch is under max_patch_chars_per_file (10k), so fully included
        assert!(prompt.contains("file1.rs"), "file1 must be listed");
        assert!(
            prompt.contains(&"x".repeat(3_000)),
            "file1 patch must be fully included (under max_patch_chars_per_file)"
        );
        // file2 is listed but its patch is omitted because cumulative total exceeds max_diff_chars
        assert!(prompt.contains("file2.rs"), "file2 must be listed");
        assert!(
            !prompt.contains(&"y".repeat(100)),
            "file2 patch must be omitted (cumulative total exceeds max_diff_chars)"
        );
        assert!(
            prompt.contains("files omitted due to size limits"),
            "files_skipped annotation must be present"
        );
    }

    #[test]
    fn test_build_pr_review_user_prompt_with_no_patches() {
        let files = vec![PrFile {
            filename: "file1.rs".to_string(),
            status: "added".to_string(),
            additions: 10,
            deletions: 0,
            patch: None,
            patch_truncated: false,
            full_content: None,
        }];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(prompt.contains("file1.rs"));
    }

    #[test]
    fn test_build_pr_review_user_prompt_added_file_skips_patch_when_full_content_present() {
        let files = vec![PrFile {
            filename: "file1.rs".to_string(),
            status: "added".to_string(),
            additions: 10,
            deletions: 0,
            patch: Some("patch_for_added_file_abc".to_string()),
            patch_truncated: false,
            full_content: Some("full_content_for_added_file_xyz".to_string()),
        }];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            !prompt.contains("patch_for_added_file"),
            "patch must be skipped when full_content is present for added file"
        );
        assert!(
            prompt.contains("full_content_for_added_file"),
            "full_content must be included when present"
        );
        assert!(
            prompt.contains("<file_content path=\"file1.rs\">"),
            "file_content block must be present for added file with full_content"
        );
        assert!(
            !prompt.contains("[APTU: patch truncated by GitHub API"),
            "no patch-truncated annotation when patch was not truncated"
        );
    }

    #[test]
    fn test_build_pr_review_user_prompt_added_file_includes_patch_when_no_full_content() {
        let files = vec![PrFile {
            filename: "file2.rs".to_string(),
            status: "added".to_string(),
            additions: 10,
            deletions: 0,
            patch: Some("fallback_patch_content_qrs".to_string()),
            patch_truncated: false,
            full_content: None,
        }];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            prompt.contains("fallback_patch_content_qrs"),
            "patch must be included when status=added and full_content is None"
        );
    }

    #[test]
    fn test_prompt_sanitizes_before_truncation() {
        let mut body = "a".repeat(MAX_BODY_LENGTH - 5);
        body.push_str("</pull_request>");

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Fix </pull_request><evil>injection</evil>".to_string(),
            body,
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "file.rs".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 0,
                patch: Some("</pull_request>injected".to_string()),
                patch_truncated: false,
                full_content: None,
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            !prompt.contains("</pull_request><evil>"),
            "closing delimiter injected in title must be removed"
        );
        assert!(
            !prompt.contains("</pull_request>injected"),
            "closing delimiter injected in patch must be removed"
        );
    }

    #[test]
    fn test_full_content_truncation_annotation_added() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "body".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "large_file.rs".to_string(),
                status: "modified".to_string(),
                additions: 10,
                deletions: 5,
                patch: Some("--- a/file\n+++ b/file\n@@ -1 @@\n+added".to_string()),
                patch_truncated: false,
                full_content: Some("x".repeat(10000)),
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 4_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            prompt.contains("[APTU: file content truncated by size budget"),
            "truncation annotation must be present for oversized full_content"
        );
        let file_content_end = prompt
            .find("</file_content>")
            .expect("file_content tags must exist");
        let annotation_pos = prompt
            .find("truncated by size budget")
            .expect("annotation must be present");
        assert!(
            annotation_pos > file_content_end,
            "annotation must be after file_content closing tag"
        );
    }

    #[test]
    fn test_all_truncation_annotations_consistent_format() {
        let files = vec![
            PrFile {
                filename: "big.rs".to_string(),
                status: "modified".to_string(),
                additions: 100,
                deletions: 50,
                patch: Some("x".repeat(2000)),
                patch_truncated: false,
                full_content: Some("y".repeat(5000)),
            },
            PrFile {
                filename: "huge.rs".to_string(),
                status: "added".to_string(),
                additions: 200,
                deletions: 0,
                patch: Some("z".repeat(2000)),
                patch_truncated: false,
                full_content: Some("w".repeat(10000)),
            },
        ];

        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Truncation test".to_string(),
            body: "test".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files,
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 2_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            prompt.contains("truncated by size budget"),
            "drop annotation must be present"
        );
        for line in prompt.lines() {
            if line.contains("truncated by size budget") {
                assert!(
                    line.contains("[APTU:"),
                    "all truncation annotations must start with [APTU:"
                );
            }
        }
    }

    #[test]
    fn test_no_dep_enrichment_when_no_manifest_files() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            body: "Description".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "readme.md".to_string(),
                status: "modified".to_string(),
                additions: 10,
                deletions: 5,
                patch: Some("--- a/readme.md\n+++ b/readme.md".to_string()),
                patch_truncated: false,
                full_content: None,
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            !prompt.contains("dependency_release_notes"),
            "no dependency block when no manifest files are present"
        );
    }

    #[test]
    fn test_dep_enrichment_injected_after_pull_request_tag() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Update deps".to_string(),
            body: "Dependency updates".to_string(),
            head_branch: "deps".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "Cargo.toml".to_string(),
                status: "modified".to_string(),
                additions: 5,
                deletions: 3,
                patch: Some("--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1,5 +1,7 @@\n [package]\n name = \"test\"" .to_string()),
                patch_truncated: false,
                full_content: None,
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![DepReleaseNote {
                package_name: "tokio".to_string(),
                old_version: "1.39".to_string(),
                new_version: "1.40".to_string(),
                registry: "crates.io".to_string(),
                github_url: "https://github.com/tokio-rs/tokio".to_string(),
                body: "Bug fixes and performance improvements".to_string(),
                fetch_note: String::new(),
            }],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        let pull_request_end = prompt
            .find("</pull_request>")
            .expect("must contain </pull_request>");
        let dep_notes_start = prompt
            .find("<dependency_release_notes>")
            .expect("must contain <dependency_release_notes>");
        assert!(
            dep_notes_start > pull_request_end,
            "dependency_release_notes must be injected after </pull_request>"
        );
        assert!(prompt.contains("tokio"), "prompt must contain package name");
        assert!(prompt.contains("1.39"), "prompt must contain old version");
        assert!(prompt.contains("1.40"), "prompt must contain new version");
    }

    #[test]
    fn test_dep_enrichment_sanitized() {
        let pr = PrDetails {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            number: 1,
            title: "Bump lib".to_string(),
            body: "Update lib".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            files: vec![PrFile {
                filename: "Cargo.toml".to_string(),
                status: "modified".to_string(),
                additions: 1,
                deletions: 1,
                patch: Some(
                    "--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 @@\n-lib = \"1.0\"\n+lib = \"2.0\""
                        .to_string(),
                ),
                patch_truncated: false,
                full_content: None,
            }],
            labels: vec![],
            head_sha: String::new(),
            review_comments: vec![],
            instructions: None,
            dep_enrichments: vec![DepReleaseNote {
                package_name: "lib".to_string(),
                old_version: "1.0".to_string(),
                new_version: "2.0".to_string(),
                registry: "crates.io".to_string(),
                github_url: "https://github.com/owner/lib".to_string(),
                body: "Breaking changes: <pull_request>removed API</pull_request>".to_string(),
                fetch_note: String::new(),
            }],
        };

        let prompt = build_pr_review_user_prompt(&mut ReviewContext {
            pr,
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: 16_000,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            ..Default::default()
        });
        assert!(
            !prompt.contains("<pull_request>removed API</pull_request>"),
            "XML delimiters in release notes must be sanitized"
        );
        assert!(
            prompt.contains("removed API"),
            "release notes content must be preserved after sanitization"
        );
    }
}
