// SPDX-License-Identifier: Apache-2.0

//! Review context policy layer for PR analysis.
//!
//! Centralizes all enrichment decisions (AST context, call graph, dependency enrichments)
//! and CWD inference into a single `ReviewContext` struct and `build_review_context()` function.

use std::path::PathBuf;

use crate::ai::types::PrDetails;

/// Review context containing all enrichment data and configuration for PR analysis.
///
/// This struct centralizes enrichment decisions and is passed to `build_pr_review_user_prompt()`
/// to avoid scattered conditional logic throughout the codebase.
#[derive(Clone, Debug)]
pub struct ReviewContext {
    /// Pull request details.
    pub pr: PrDetails,
    /// AST context for changed files (empty if not available or feature disabled).
    pub ast_context: String,
    /// Call graph context for changed files (empty if not available or feature disabled).
    pub call_graph: String,
    /// Inferred repository path from CWD (if available).
    pub inferred_repo_path: Option<PathBuf>,
    /// Whether the repository path was inferred from CWD.
    pub cwd_inferred: bool,
    /// Maximum characters per file's full content in the prompt (from `ReviewConfig`).
    pub max_chars_per_file: usize,
    /// Maximum total diff characters across all files in the prompt (from `ReviewConfig`).
    pub max_diff_chars: usize,
    /// Maximum characters per individual file patch before the patch is dropped entirely (from `ReviewConfig`).
    pub max_patch_chars_per_file: usize,
    /// Number of files whose full content was truncated at prompt assembly.
    pub files_truncated: usize,
    /// Total characters dropped across all truncated files.
    pub truncated_chars_dropped: usize,
    /// Names of files whose patches were truncated or skipped at prompt assembly.
    pub truncated_patch_files: Vec<String>,
    /// Total number of files in the PR.
    pub files_total: usize,
    /// Number of files with a patch (non-empty diff).
    pub files_with_patch: usize,
    /// Number of dependency enrichments applied.
    pub dep_enrichments_count: usize,
    /// Total characters in dependency enrichments.
    pub dep_enrichments_chars: usize,
    /// Names of context items dropped due to budget constraints.
    pub budget_drops: Vec<String>,
    /// Final assembled prompt character count.
    pub prompt_chars_final: usize,
    /// Estimated total character size of the PR review prompt before budget drops.
    pub estimated_size: usize,
}

impl ReviewContext {
    /// Returns a formatted pre-flight summary for verbose output.
    ///
    /// Includes package names, character counts, and CWD inference status.
    #[must_use]
    pub fn verbose_summary(&self) -> String {
        use std::fmt::Write;

        let mut summary = String::new();

        // Repo path info
        if let Some(path) = &self.inferred_repo_path {
            let inferred_label = if self.cwd_inferred { " (inferred)" } else { "" };
            let _ = writeln!(
                summary,
                "Repository path: {}{}",
                path.display(),
                inferred_label
            );
        }

        // Enrichment summary
        if !self.pr.dep_enrichments.is_empty() {
            let packages: Vec<&str> = self
                .pr
                .dep_enrichments
                .iter()
                .map(|d| d.package_name.as_str())
                .collect();
            let _ = writeln!(summary, "Dependency enrichments: {}", packages.join(", "));
        }

        // Context sizes
        let mut context_sizes = Vec::new();
        if !self.ast_context.is_empty() {
            context_sizes.push(format!("AST: {} chars", self.ast_context.len()));
        }
        if !self.call_graph.is_empty() {
            context_sizes.push(format!("call graph: {} chars", self.call_graph.len()));
        }
        if !context_sizes.is_empty() {
            let _ = writeln!(summary, "Context: {}", context_sizes.join(", "));
        }

        // Truncation summary
        if self.files_truncated > 0 {
            let _ = writeln!(
                summary,
                "Files truncated: {} ({} chars dropped)",
                self.files_truncated, self.truncated_chars_dropped
            );
        }

        summary
    }

    /// Records a file truncation event.
    ///
    /// Updates truncation counters and emits a debug log.
    pub fn record_truncation(&mut self, filename: &str, original_len: usize, truncated_len: usize) {
        self.files_truncated += 1;
        self.truncated_chars_dropped += original_len - truncated_len;
        tracing::debug!(
            filename = %filename,
            original_len,
            truncated_len,
            "file content truncated at prompt assembly"
        );
    }
}

impl Default for ReviewContext {
    fn default() -> Self {
        Self {
            pr: crate::ai::types::PrDetails {
                owner: String::new(),
                repo: String::new(),
                number: 0,
                title: String::new(),
                body: String::new(),
                base_branch: String::new(),
                head_branch: String::new(),
                files: Vec::new(),
                url: String::new(),
                labels: Vec::new(),
                head_sha: String::new(),
                review_comments: Vec::new(),
                instructions: None,
                dep_enrichments: Vec::new(),
            },
            ast_context: String::new(),
            call_graph: String::new(),
            inferred_repo_path: None,
            cwd_inferred: false,
            max_chars_per_file: crate::config::ReviewConfig::default().max_chars_per_file,
            max_diff_chars: crate::config::ReviewConfig::default().max_diff_chars,
            max_patch_chars_per_file: crate::config::ReviewConfig::default()
                .max_patch_chars_per_file,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            truncated_patch_files: Vec::new(),
            files_total: 0,
            files_with_patch: 0,
            dep_enrichments_count: 0,
            dep_enrichments_chars: 0,
            budget_drops: Vec::new(),
            prompt_chars_final: 0,
            estimated_size: 0,
        }
    }
}

mod budget;
mod builder;
mod truncation;

pub use builder::build_review_context;
pub(crate) use truncation::truncate_at_line_boundary;

#[allow(unused_imports)]
pub(crate) use budget::drop_full_content_by_size;
#[allow(unused_imports)]
pub(crate) use budget::should_enable_call_graph;
#[allow(unused_imports)]
pub(crate) use budget::{
    PROMPT_OVERHEAD_CHARS, apply_budget_drops, build_file_outline, estimate_pr_size,
};
#[allow(unused_imports)]
pub(crate) use builder::{build_ctx_ast, build_ctx_call_graph, enrich_deps};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types::{DepReleaseNote, PrFile};
    use crate::config::ReviewConfig;

    fn make_pr_with_content(patch_chars: usize, full_content_chars: usize) -> PrDetails {
        PrDetails {
            number: 1,
            title: "test".to_string(),
            body: String::new(),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            url: "https://github.com/owner/repo/pull/1".to_string(),
            head_branch: "feat".to_string(),
            base_branch: "main".to_string(),
            head_sha: String::new(),
            review_comments: vec![],
            files: vec![PrFile {
                filename: "src/lib.rs".to_string(),
                status: "modified".to_string(),
                patch: Some("x".repeat(patch_chars)),
                patch_truncated: false,
                full_content: if full_content_chars > 0 {
                    Some("y".repeat(full_content_chars))
                } else {
                    None
                },
                additions: 1,
                deletions: 0,
            }],
            dep_enrichments: vec![],
            instructions: None,
            labels: vec![],
        }
    }

    fn make_dep(package_name: &str) -> DepReleaseNote {
        DepReleaseNote {
            package_name: package_name.to_string(),
            old_version: "1.0.0".to_string(),
            new_version: "1.0.1".to_string(),
            registry: "crates.io".to_string(),
            github_url: format!("https://github.com/owner/{package_name}"),
            body: "release notes".to_string(),
            fetch_note: String::new(),
        }
    }

    /// Verifies that `apply_budget_drops` enforces the documented drop order:
    /// `call_graph` -> `ast_context` -> `dep_enrichments` -> `patches` -> `full_content`.
    #[test]
    fn test_apply_budget_drops_order() {
        let mut pr = make_pr_with_content(500, 500);
        let mut ast_context = "a".repeat(300);
        let mut call_graph = "b".repeat(300);

        // Budget tight enough that call_graph must be dropped first.
        // Total with all: patch(500) + full_content(500) + ast(300) + call_graph(300)
        //                + metadata(~30) + PROMPT_OVERHEAD_CHARS(1000) = ~2630
        // Set budget to force call_graph drop.
        let max_prompt_chars = 600;

        let mut drops = Vec::new();
        // Priority order: call_graph -> ast_context -> dep_enrichments -> patches -> full_content
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // call_graph dropped first (over budget)
        assert!(
            call_graph.is_empty(),
            "call_graph should be dropped first when over budget"
        );
    }

    /// Verifies that `dep_enrichments` are dropped before file patches.
    #[test]
    fn test_apply_budget_drops_dep_enrichments_before_patches() {
        let mut pr = make_pr_with_content(200, 0);
        // Add a large dep enrichment body to make it over budget
        pr.dep_enrichments.push(make_dep("serde"));
        pr.dep_enrichments[0].body = "d".repeat(400);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Budget: just under (patch + dep_body + overhead) to force dep drop but not patch drop.
        // Base estimate (without dep): patch(200) + metadata(~30) + PROMPT_OVERHEAD_CHARS(1000) = ~1230
        // With dep: + package_name(6) + body(400) + github_url(~30) = ~1666
        // Budget between 1230 and 1666 so dep is dropped but patch is retained.
        let max_prompt_chars = 1400;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // dep_enrichments dropped before patches
        assert!(
            pr.dep_enrichments.is_empty(),
            "dep_enrichments should be dropped before file patches"
        );
        // patch should still be present (dep drop was enough to fit)
        assert!(
            pr.files[0].patch.is_some(),
            "file patch should be retained when dep drop brought size within budget"
        );
    }

    /// Verifies that empty sections do NOT appear in `budget_drops` telemetry.
    /// Regression test for telemetry accuracy bug where empty-section drops were unconditionally recorded.
    #[test]
    fn test_apply_budget_drops_empty_sections() {
        let mut pr = make_pr_with_content(100, 100);
        // All context sections are empty -- no content to drop
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Tight budget to trigger drop attempts, but sections are empty
        let max_prompt_chars = 500;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Empty sections should NOT appear in budget_drops
        // (they were cleared but no content was actually dropped)
        assert!(
            !drops.contains(&"call_graph".to_string()),
            "empty call_graph should not appear in budget_drops"
        );
        assert!(
            !drops.contains(&"ast_context".to_string()),
            "empty ast_context should not appear in budget_drops"
        );
    }

    /// Verifies that populated sections DO appear in `budget_drops` telemetry.
    /// Ensures the fix does not suppress telemetry for sections with actual content.
    #[test]
    fn test_apply_budget_drops_populated_sections() {
        let mut pr = make_pr_with_content(100, 100);
        // Populate all context sections with meaningful content
        let mut ast_context = "a".repeat(300);
        let mut call_graph = "b".repeat(300);

        // Tight budget to force drops, with populated sections
        let max_prompt_chars = 600;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Populated sections should appear in budget_drops (in priority order)
        // Priority order: call_graph -> ast_context
        assert!(
            drops.contains(&"call_graph".to_string()),
            "populated call_graph should appear in budget_drops"
        );
        // ast_context presence depends on budget constraints,
        // but at least call_graph must be present for this test
    }

    /// Regression test for issue #1548: verifies patch and `full_content` budget drops
    /// use distinct prefixes ("patch:" vs "`file_content`:") to avoid collision.
    #[test]
    fn test_budget_drops_distinguish_patch_from_full_content() {
        let mut pr = make_pr_with_content(500, 500);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Budget tight enough to force BOTH patch and full_content drops for the file
        let max_prompt_chars = 50;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        // Both patch and full_content should be None (both dropped)
        assert!(
            pr.files[0].patch.is_none(),
            "patch should be dropped when over budget"
        );
        assert!(
            pr.files[0].full_content.is_none(),
            "full_content should be dropped when over budget"
        );

        // Drops should contain exactly two distinct entries: one for patch, one for full_content
        assert!(
            drops.contains(&"patch:src/lib.rs".to_string()),
            "budget_drops should contain 'patch:src/lib.rs' for the dropped patch"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should contain 'file_content:src/lib.rs' for the dropped full_content"
        );

        // Verify they are distinct (not duplicates with the same prefix)
        let patch_drops: Vec<_> = drops.iter().filter(|s| s.starts_with("patch:")).collect();
        let full_content_drops: Vec<_> = drops
            .iter()
            .filter(|s| s.starts_with("file_content:"))
            .collect();
        assert_eq!(
            patch_drops.len(),
            1,
            "should have exactly one 'patch:' entry"
        );
        assert_eq!(
            full_content_drops.len(),
            1,
            "should have exactly one 'file_content:' entry"
        );
    }

    /// Creates a temp directory containing a single fixture file, for tests that
    /// need `analyze_file` to run against a real file on disk.
    #[cfg(feature = "ast-context")]
    fn make_outline_fixture(filename: &str, source: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::write(dir.path().join(filename), source).expect("failed to write fixture file");
        dir
    }

    /// Verifies that when the signature outline fits within the remaining budget,
    /// `drop_full_content_by_size` substitutes it into `full_content` and records
    /// the `file_content_outline:` label instead of clearing the file entirely.
    #[cfg(feature = "ast-context")]
    #[test]
    fn test_drop_full_content_by_size_outline_fits_budget() {
        let dir = make_outline_fixture(
            "fixture.rs",
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        );
        let repo_path = dir.path().to_string_lossy().into_owned();

        let mut files = vec![crate::ai::types::PrFile {
            filename: "fixture.rs".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(2000)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 2000;
        let max_prompt_chars = 100;
        let mut drops = Vec::new();

        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some(&repo_path),
        );

        assert!(
            files[0]
                .full_content
                .as_deref()
                .is_some_and(|c| c.contains("fn add")),
            "full_content should be replaced with a signature outline"
        );
        assert!(
            drops.contains(&"file_content_outline:fixture.rs".to_string()),
            "budget_drops should record the outline substitution"
        );
    }

    /// Verifies that when the outline itself still exceeds the remaining budget,
    /// `drop_full_content_by_size` falls back to fully clearing `full_content`
    /// without panicking.
    #[test]
    fn test_drop_full_content_by_size_outline_still_exceeds_budget() {
        let mut files = vec![crate::ai::types::PrFile {
            filename: "src/lib.rs".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(2000)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 2000;
        // A zero budget can never be satisfied by any non-empty outline, so this
        // exercises the fallback regardless of whether the ast-context feature
        // is enabled.
        let max_prompt_chars = 0;
        let mut drops = Vec::new();

        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some("/nonexistent/repo/path"),
        );

        assert!(
            files[0].full_content.is_none(),
            "full_content should be fully cleared when the outline cannot fit"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should record the full clear, not an outline substitution"
        );
    }

    /// Verifies that an unsupported file extension skips `analyze_file` entirely
    /// and falls back to fully clearing `full_content`.
    #[test]
    fn test_drop_full_content_by_size_unsupported_extension_falls_back() {
        let mut files = vec![crate::ai::types::PrFile {
            filename: "data.unsupportedext".to_string(),
            status: "modified".to_string(),
            patch: None,
            patch_truncated: false,
            full_content: Some("y".repeat(200)),
            additions: 1,
            deletions: 0,
        }];
        let mut estimated_size = 200;
        let max_prompt_chars = 50;
        let mut drops = Vec::new();

        // repo_path points at a directory that does not exist; if analyze_file were
        // called, it would error, but the unsupported extension must short-circuit
        // before that call happens.
        drop_full_content_by_size(
            &mut files,
            &mut estimated_size,
            max_prompt_chars,
            &mut drops,
            Some("/nonexistent/repo/path"),
        );

        assert!(
            files[0].full_content.is_none(),
            "full_content should be cleared for unsupported extensions"
        );
        assert!(
            drops.contains(&"file_content:data.unsupportedext".to_string()),
            "budget_drops should record the full clear for the unsupported extension"
        );
    }

    /// Automated benchmark for issue #1592: measures the outline's actual size
    /// reduction against this crate's own real source files (not synthetic
    /// fixtures), confirming the substitution meaningfully shrinks prompt size
    /// on typical Rust files with real function counts and import lists.
    #[cfg(feature = "ast-context")]
    #[test]
    fn test_outline_size_reduction_on_real_source_files() {
        let repo_path = env!("CARGO_MANIFEST_DIR");
        let candidates = [
            "src/ai/review_context.rs",
            "src/ast_context.rs",
            "src/ai/types.rs",
        ];

        let mut measured = 0;
        for filename in candidates {
            let full_path = std::path::Path::new(repo_path).join(filename);
            let Ok(full_content) = std::fs::read_to_string(&full_path) else {
                continue;
            };
            let Some(outline) = build_file_outline(repo_path, filename) else {
                continue;
            };

            let reduction_pct = 100 - (outline.len() * 100 / full_content.len().max(1));
            println!(
                "{filename}: full_content={} chars, outline={} chars, reduction={reduction_pct}%",
                full_content.len(),
                outline.len()
            );

            assert!(
                outline.len() < full_content.len() / 2,
                "{filename}: outline ({} chars) should be well under half of full content ({} chars)",
                outline.len(),
                full_content.len()
            );
            measured += 1;
        }

        assert!(
            measured > 0,
            "expected at least one real source file to produce a measurable outline"
        );
    }

    /// Regression test for issue #1596: `full_content` must be evicted before patches,
    /// so a PR with a small diff but a large `full_content` still keeps its patch.
    #[test]
    fn test_apply_budget_drops_full_content_evicted_before_patch() {
        // patch is small, full_content alone is large enough to blow the budget.
        let mut pr = make_pr_with_content(50, 1000);
        let mut ast_context = String::new();
        let mut call_graph = String::new();

        // Base without full_content: patch(50) + metadata(~30) + overhead(1000) = ~1080.
        // With full_content: + 1000 = ~2080. Budget sits between the two, so only
        // full_content needs dropping to fit; the patch must survive untouched.
        let max_prompt_chars = 1500;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        assert!(
            pr.files[0].full_content.is_none(),
            "full_content should be evicted first when over budget"
        );
        assert!(
            pr.files[0].patch.is_some(),
            "patch should survive when evicting full_content alone brings the prompt under budget"
        );
        assert!(
            drops.contains(&"file_content:src/lib.rs".to_string()),
            "budget_drops should record the file_content eviction"
        );
        assert!(
            !drops.contains(&"patch:src/lib.rs".to_string()),
            "budget_drops should not record a patch eviction"
        );
    }

    /// Regression test for issue #1596 review feedback: when both `full_content` and
    /// patches must be dropped across multiple files, `budget_drops` must record every
    /// `file_content:` entry before any `patch:` entry, matching the call order.
    #[test]
    fn test_apply_budget_drops_records_full_content_before_patch_order() {
        let mut pr = make_pr_with_content(500, 100);
        pr.files.push(PrFile {
            filename: "src/b.rs".to_string(),
            status: "modified".to_string(),
            patch: Some("x".repeat(500)),
            patch_truncated: false,
            full_content: Some("y".repeat(100)),
            additions: 1,
            deletions: 0,
        });
        let mut ast_context = String::new();
        let mut call_graph = String::new();
        let max_prompt_chars = 1900;

        let mut drops = Vec::new();
        apply_budget_drops(
            &mut pr,
            &mut ast_context,
            &mut call_graph,
            max_prompt_chars,
            &mut drops,
            None,
        );

        let last_file_content_idx = drops.iter().rposition(|d| d.starts_with("file_content:"));
        let first_patch_idx = drops.iter().position(|d| d.starts_with("patch:"));
        assert!(
            last_file_content_idx.is_some() && first_patch_idx.is_some(),
            "expected both file_content and patch drops in this scenario: {drops:?}"
        );
        assert!(
            last_file_content_idx < first_patch_idx,
            "every file_content: entry must precede every patch: entry in budget_drops: {drops:?}"
        );
    }

    #[test]
    fn test_verbose_summary_all_fields() {
        // Arrange: ReviewContext with repo path (inferred), dep enrichments, ast, call graph
        let mut pr = make_pr_with_content(10, 0);
        pr.dep_enrichments = vec![make_dep("tokio"), make_dep("serde")];
        let ctx = ReviewContext {
            pr,
            ast_context: "fn foo() {}".to_string(),
            call_graph: "foo -> bar".to_string(),
            inferred_repo_path: Some(std::path::PathBuf::from("/tmp/repo")),
            cwd_inferred: true,
            ..Default::default()
        };

        // Act
        let summary = ctx.verbose_summary();

        // Assert: repo path with inferred label
        assert!(
            summary.contains("/tmp/repo"),
            "summary should contain the repo path"
        );
        assert!(
            summary.contains("(inferred)"),
            "summary should mark CWD-inferred path"
        );
        // Assert: dep package names
        assert!(
            summary.contains("tokio"),
            "summary should list dep package names"
        );
        assert!(
            summary.contains("serde"),
            "summary should list dep package names"
        );
        // Assert: context sizes
        assert!(
            summary.contains("AST:"),
            "summary should include AST char count"
        );
        assert!(
            summary.contains("call graph:"),
            "summary should include call graph char count"
        );
    }

    #[test]
    fn test_verbose_summary_empty_context() {
        // Arrange: ReviewContext with no enrichments and no repo path
        let pr = make_pr_with_content(0, 0);
        let ctx = ReviewContext {
            pr,
            ..Default::default()
        };

        // Act
        let summary = ctx.verbose_summary();

        // Assert: nothing to report means empty string
        assert!(
            summary.is_empty(),
            "summary should be empty when no enrichments are present"
        );
    }

    #[test]
    fn test_verbose_summary_truncation_section_present_and_absent() {
        // Arrange
        let pr = make_pr_with_content(0, 0);

        // Case 1: files_truncated > 0 -- section must be present
        let ctx_with = ReviewContext {
            pr: pr.clone(),
            max_chars_per_file: 4_000,
            files_truncated: 3,
            truncated_chars_dropped: 900,
            ..Default::default()
        };
        let summary = ctx_with.verbose_summary();
        assert!(
            summary.contains("Files truncated: 3 (900 chars dropped)"),
            "verbose_summary must include truncation line when files_truncated > 0"
        );

        // Case 2: files_truncated == 0 -- section must be absent
        let ctx_without = ReviewContext {
            pr,
            max_chars_per_file: 4_000,
            ..Default::default()
        };
        let summary_clean = ctx_without.verbose_summary();
        assert!(
            !summary_clean.contains("Files truncated"),
            "verbose_summary must omit truncation line when files_truncated == 0"
        );
    }

    #[test]
    fn test_should_enable_call_graph_budget_boundary() {
        // budget_remaining == min_budget_for_call_graph -> false (strict >)
        let config = ReviewConfig {
            min_budget_for_call_graph: 20_000,
            ..ReviewConfig::default()
        };
        assert!(
            !should_enable_call_graph(20_000, &config),
            "should_enable_call_graph must be false when budget_remaining equals min_budget_for_call_graph"
        );
    }

    #[test]
    fn test_should_enable_call_graph_budget_below_threshold() {
        // budget_remaining < min_budget_for_call_graph -> false
        let config = ReviewConfig {
            min_budget_for_call_graph: 20_000,
            ..ReviewConfig::default()
        };
        assert!(
            !should_enable_call_graph(10_000, &config),
            "should_enable_call_graph must be false when budget_remaining < min_budget_for_call_graph"
        );
    }

    // -----------------------------------------------------------------------
    // truncate_at_line_boundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_truncate_at_line_boundary_happy_path() {
        // Content with newlines; truncation should land on last newline before limit
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        // max_chars=20: chars "line 1\nline 2\nlin" -> find last '\n' -> "line 1\nline 2\n"
        let result = truncate_at_line_boundary(content, 20);
        assert_eq!(result, "line 1\nline 2\n");
        assert!(
            result.chars().count() <= 20,
            "truncated result must not exceed max_chars"
        );
        assert!(
            result.ends_with('\n'),
            "truncation should end at newline boundary when one exists"
        );
    }

    #[test]
    fn test_truncate_at_line_boundary_fallback_no_newline() {
        // Content with no newline; must fall back to char boundary
        let content = "abcdefghijklmnopqrstuvwxyz";
        let result = truncate_at_line_boundary(content, 10);
        assert_eq!(result, "abcdefghij");
        assert_eq!(result.chars().count(), 10);
    }

    #[test]
    fn test_truncate_at_line_boundary_under_limit() {
        // Content within limit should be returned unchanged
        let content = "short";
        let result = truncate_at_line_boundary(content, 100);
        assert_eq!(result, "short");
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_truncate_at_line_boundary_multi_byte_utf8() {
        // Multi-byte UTF-8 characters before the cut; must not panic
        let content: String = (0..30).map(|_| "\u{1F600}").collect(); // 30 emoji chars
        let result = truncate_at_line_boundary(&content, 25);
        // 25 chars from 30 should give 25 emoji chars (no newline, so char boundary fallback)
        assert_eq!(result.chars().count(), 25);
        // Every char should be the emoji
        assert!(result.chars().all(|c| c == '\u{1F600}'));
    }

    #[test]
    fn test_estimate_pr_size_includes_call_graph() {
        // Verify estimate_pr_size includes call_graph chars and PROMPT_OVERHEAD_CHARS
        let pr = make_pr_with_content(0, 0);
        let ast_context = "";
        let call_graph = "fn foo() -> bar\nfn baz() -> qux";
        let size = estimate_pr_size(&pr, ast_context, call_graph);
        let without_call_graph = estimate_pr_size(&pr, ast_context, "");
        // Delta between with and without call_graph should be exactly call_graph.len()
        assert_eq!(size - without_call_graph, call_graph.len());
        // Total should include PROMPT_OVERHEAD_CHARS
        assert!(size >= PROMPT_OVERHEAD_CHARS);
    }

    #[test]
    fn test_build_review_context_estimated_size_pre_budget() {
        // Verify estimate_pr_size accounts for call_graph + overhead before budget drops
        // using a non-minimal PrDetails with patches and full_content
        let pr = make_pr_with_content(50, 100);
        let ast_context = "fn foo() {}";
        let call_graph = "caller -> callee\nother -> thing";
        let size = estimate_pr_size(&pr, ast_context, call_graph);
        assert!(
            size >= call_graph.len() + PROMPT_OVERHEAD_CHARS,
            "estimated size {} should be >= call_graph.len() {} + overhead {}",
            size,
            call_graph.len(),
            PROMPT_OVERHEAD_CHARS
        );
    }

    /// Regression test for #1571 (structural graph removal): `build_review_context()`
    /// no longer threads a `GraphConfig` or builds `graph_context`/`graph_cache_hit`, but
    /// its `ast_context` and `call_graph` enrichment paths must be completely unaffected
    /// by that removal.
    #[tokio::test]
    async fn test_build_review_context_preserves_ast_and_call_graph() {
        // Arrange: a fixture PR touching a real Rust source file in this crate, with
        // min_budget_for_call_graph=0 to force call_graph building regardless of prompt
        // budget (mirrors the fixture setup pattern used by ast_context.rs's own
        // build_ast_context tests).
        let repo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let mut pr = make_pr_with_content(0, 0);
        pr.files = vec![PrFile {
            filename: "src/ast_context.rs".to_string(),
            status: "modified".to_string(),
            patch: Some("+// touched".to_string()),
            patch_truncated: false,
            full_content: None,
            additions: 1,
            deletions: 0,
        }];
        let review_config = ReviewConfig {
            min_budget_for_call_graph: 0,
            ..ReviewConfig::default()
        };

        // Act
        let ctx = build_review_context(pr, Some(repo_path), &review_config)
            .await
            .expect("build_review_context should succeed for a valid repo_path");

        // Assert: shape is exactly what the standalone builders produce -- empty when
        // the `ast-context` feature is off, well-formed XML-tagged output when it's on.
        assert!(
            ctx.ast_context.is_empty() || ctx.ast_context.contains("<ast_context>"),
            "ast_context should be empty or well-formed XML-tagged output"
        );
        assert!(
            ctx.call_graph.is_empty() || ctx.call_graph.contains("<call_graph>"),
            "call_graph should be empty or well-formed XML-tagged output"
        );

        #[cfg(feature = "ast-context")]
        assert!(
            !ctx.ast_context.is_empty(),
            "ast_context should be populated for a real Rust fixture file when the ast-context feature is enabled"
        );
    }

    /// Regression test for issue #1596: `build_review_context` must refuse to produce
    /// a diff-less review when a non-empty PR ends up with zero surviving patches
    /// after budget drops (which would trigger a GitHub HTTP 422 downstream).
    #[tokio::test]
    async fn test_build_review_context_errs_when_all_patches_evicted() {
        // Arrange: a single file with a sizable patch and full_content, and a budget
        // so tiny that every section (including the patch, as a last resort) is dropped.
        let pr = make_pr_with_content(2000, 2000);
        let review_config = ReviewConfig {
            max_prompt_chars: 1,
            ..ReviewConfig::default()
        };

        // Act: repo_path points nowhere so ast_context/call_graph building is a no-op.
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            &review_config,
        )
        .await;

        // Assert
        match result {
            Err(crate::AptuError::EmptyReviewContext { files_total }) => {
                assert_eq!(
                    files_total, 1,
                    "files_total should reflect the single PR file"
                );
            }
            other => panic!("expected EmptyReviewContext error, got {other:?}"),
        }
    }

    /// Regression test for issue #1596: an empty PR (no files) must not trigger the
    /// zero-patch guard, since `files_with_patch == 0` is vacuously true there.
    #[tokio::test]
    async fn test_build_review_context_no_err_when_files_empty() {
        // Arrange: a PR with zero files.
        let mut pr = make_pr_with_content(0, 0);
        pr.files = vec![];
        let review_config = ReviewConfig {
            max_prompt_chars: 1,
            ..ReviewConfig::default()
        };

        // Act
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            &review_config,
        )
        .await;

        // Assert
        assert!(
            result.is_ok(),
            "an empty-file PR should not trigger the zero-patch guard: {result:?}"
        );
    }

    /// Regression test for issue #1596: a non-empty PR where no file ever had a
    /// patch (e.g. binary-only content) must not trigger the zero-patch guard,
    /// since `had_patches_pre_drop` is false rather than an eviction outcome.
    #[tokio::test]
    async fn test_build_review_context_no_err_when_no_patches_pre_drop() {
        // Arrange: a single file with no patch and no full_content from the start,
        // simulating a binary-only file with no diffable content.
        let mut pr = make_pr_with_content(0, 0);
        pr.files[0].patch = None;
        let review_config = ReviewConfig::default();

        // Act: repo_path points nowhere so ast_context/call_graph building is a no-op.
        let result = build_review_context(
            pr,
            Some("/nonexistent-aptu-test-repo-path".to_string()),
            &review_config,
        )
        .await;

        // Assert
        assert!(
            result.is_ok(),
            "a non-empty PR with no pre-drop patches (binary-only) should not trigger the zero-patch guard: {result:?}"
        );
    }
}
