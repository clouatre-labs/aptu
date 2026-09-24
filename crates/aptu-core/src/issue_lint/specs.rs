// SPDX-License-Identifier: Apache-2.0

//! Spec loading, resolution, and the pure `lint_issue` entry point.
//!
//! Resolution order: explicit `--config` file, then repo-root
//! `issue-lint-specs.toml` (auto-discovered from the working directory),
//! then built-in generic checks.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::issue_lint::parser::{
    extract_h2_headings, has_code_example, has_external_reference, has_fenced_code,
    strip_fenced_blocks, strip_html_comments,
};
use crate::issue_lint::types::{IssueLintResult, IssueLintSpec, IssueLintViolation};

/// File name auto-discovered at the repository root.
pub const SPECS_FILE_NAME: &str = "issue-lint-specs.toml";

/// Minimum trimmed body length (chars) for the generic length check.
const GENERIC_MIN_BODY_CHARS: usize = 100;

/// Minimum checkbox count in an acceptance-criteria-like section.
const GENERIC_MIN_CHECKBOXES: usize = 2;

/// Root shape of a repository `issue-lint-specs.toml` file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecsFile {
    /// Per-template specs.
    #[serde(default, rename = "spec")]
    pub specs: Vec<IssueLintSpec>,
}

/// Where the effective lint spec came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecResolution {
    /// Specs loaded from an explicit config file.
    Explicit(Vec<IssueLintSpec>),
    /// Specs auto-discovered at the repository root.
    RepoRoot(Vec<IssueLintSpec>),
    /// No spec found; generic checks apply regardless of `--type`.
    Generic,
}

/// Loads specs from a TOML file path.
///
/// # Errors
/// Returns an error when the file cannot be read or is not valid TOML
/// matching the `[[spec]]` schema.
pub fn load_specs(path: &Path) -> Result<Vec<IssueLintSpec>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read lint config {}", path.display()))?;
    parse_specs(&content).with_context(|| format!("invalid lint config {}", path.display()))
}

/// Parses specs from a TOML string.
///
/// # Errors
/// Returns an error when the content is not valid TOML matching the schema.
pub fn parse_specs(content: &str) -> Result<Vec<IssueLintSpec>> {
    let file: SpecsFile = toml::from_str(content).with_context(|| "failed to parse TOML specs")?;
    Ok(file.specs)
}

/// Resolves which specs apply, in precedence order: explicit config,
/// repo-root auto-discovery, generic mode.
///
/// # Errors
/// Returns an error when an explicit config is unreadable or malformed.
pub fn resolve_specs(explicit_config: Option<&Path>) -> Result<SpecResolution> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_specs_in(&cwd, explicit_config)
}

/// Walks upward from `start` looking for a `.git` entry (file or directory,
/// covering worktrees and submodules); returns the first ancestor containing
/// one, or `start` itself when no repository root is found.
#[must_use]
pub fn find_repo_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(start)
        .to_path_buf()
}

/// Like [`resolve_specs`] but auto-discovers `issue-lint-specs.toml` at the
/// repository root found by walking upward from `base` (testable).
///
/// # Errors
/// Returns an error when a chosen config is unreadable or malformed.
pub fn resolve_specs_in(base: &Path, explicit_config: Option<&Path>) -> Result<SpecResolution> {
    if let Some(path) = explicit_config {
        return Ok(SpecResolution::Explicit(load_specs(path)?));
    }
    let repo_root = find_repo_root(base).join(SPECS_FILE_NAME);
    if repo_root.is_file() {
        return Ok(SpecResolution::RepoRoot(load_specs(&repo_root)?));
    }
    Ok(SpecResolution::Generic)
}

/// Finds the spec matching `issue_type` in a loaded spec set.
#[must_use]
pub fn find_spec<'a>(specs: &'a [IssueLintSpec], issue_type: &str) -> Option<&'a IssueLintSpec> {
    specs.iter().find(|s| s.issue_type == issue_type)
}

/// Lints an issue body. `None` spec means generic mode.
#[must_use]
pub fn lint_issue(body: &str, spec: Option<&IssueLintSpec>) -> IssueLintResult {
    match spec {
        Some(spec) => lint_with_spec(body, spec),
        None => lint_generic(body),
    }
}

/// Lints the body against an explicit spec (headings + optional checks).
fn lint_with_spec(body: &str, spec: &IssueLintSpec) -> IssueLintResult {
    let mut violations = Vec::new();
    let headings = extract_h2_headings(body);
    for required in &spec.required_headings {
        if !headings.iter().any(|(name, _)| name == required) {
            violations.push(IssueLintViolation {
                rule: "spec/missing-heading".to_string(),
                message: format!("missing required heading: {required}"),
                line: None,
            });
        }
    }
    if spec.require_code_examples && !has_code_example(body) {
        violations.push(IssueLintViolation {
            rule: "spec/no-code-example".to_string(),
            message: "no code fence or file-path reference found".to_string(),
            line: None,
        });
    }
    if spec.require_external_link
        && !has_external_reference(&strip_fenced_blocks(&strip_html_comments(body)))
    {
        violations.push(IssueLintViolation {
            rule: "spec/no-external-link".to_string(),
            message: "no external URL or #N issue reference found".to_string(),
            line: None,
        });
    }
    IssueLintResult {
        passed: violations.is_empty(),
        violations,
    }
}

/// Lints the body with the four generic deterministic checks.
fn lint_generic(body: &str) -> IssueLintResult {
    // All checks run on stripped text (HTML comments and fenced contents
    // removed) except fence detection, which needs the raw body.
    let stripped = strip_fenced_blocks(&strip_html_comments(body));
    let mut violations = Vec::new();
    let trimmed = stripped.trim();
    if trimmed.chars().count() < GENERIC_MIN_BODY_CHARS {
        violations.push(IssueLintViolation {
            rule: "generic/body-too-short".to_string(),
            message: format!(
                "body is shorter than the {GENERIC_MIN_BODY_CHARS}-character one-liner floor"
            ),
            line: None,
        });
    }
    let checkbox_count = acceptance_checkbox_count(&stripped);
    if checkbox_count < GENERIC_MIN_CHECKBOXES {
        violations.push(IssueLintViolation {
            rule: "generic/acceptance-criteria".to_string(),
            message: format!(
                "acceptance-criteria-like section needs at least {GENERIC_MIN_CHECKBOXES} checkboxes (found {checkbox_count})"
            ),
            line: None,
        });
    }
    if !(has_fenced_code(body) || has_code_example(&stripped)) {
        violations.push(IssueLintViolation {
            rule: "generic/no-code-example".to_string(),
            message: "no code fence or file-path reference found".to_string(),
            line: None,
        });
    }
    if !has_external_reference(&stripped) {
        violations.push(IssueLintViolation {
            rule: "generic/no-external-link".to_string(),
            message: "no external URL or #N issue reference found".to_string(),
            line: None,
        });
    }
    IssueLintResult {
        passed: violations.is_empty(),
        violations,
    }
}

/// Counts `- [ ]` / `- [x]` checkboxes under an acceptance-criteria-like
/// heading (heading name containing "acceptance", case-insensitive).
fn acceptance_checkbox_count(body: &str) -> usize {
    let mut in_section = false;
    let mut count = 0;
    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.to_lowercase().contains("acceptance");
        } else if in_section {
            let lower = line.trim_start().to_lowercase();
            if lower.starts_with("- [ ]") || lower.starts_with("- [x]") {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature_spec() -> IssueLintSpec {
        IssueLintSpec {
            issue_type: "feature".to_string(),
            required_headings: vec![
                "Summary".to_string(),
                "Context".to_string(),
                "Acceptance Criteria".to_string(),
            ],
            require_code_examples: true,
            require_external_link: true,
        }
    }

    fn conforming_body() -> String {
        [
            "## Summary",
            "Add a deterministic lint operation for issue bodies with enough detail to act on.",
            "## Context",
            "Follows the scan-security pattern described in docs/SECURITY_SCANNING.md (#1702).",
            "## Acceptance Criteria",
            "- [ ] passes on conforming bodies",
            "- [ ] fails with named headings",
            "",
        ]
        .join("\n")
    }

    /// Conformant feature body yields zero violations.
    #[test]
    fn test_lint_issue_conformant_body_passes() {
        // Arrange
        let body = conforming_body();

        // Act
        let result = lint_issue(&body, Some(&feature_spec()));

        // Assert
        assert!(result.passed, "violations: {:?}", result.violations);
    }

    /// Each missing required heading produces exactly one violation.
    #[test]
    fn test_lint_issue_missing_headings() {
        // Arrange
        let body = "## Summary\nsome summary text that is reasonably long for a body\n";

        // Act
        let result = lint_issue(body, Some(&feature_spec()));

        // Assert
        assert!(!result.passed);
        let missing: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.rule == "spec/missing-heading")
            .collect();
        assert_eq!(missing.len(), 2);
        assert!(
            missing
                .iter()
                .any(|v| v.message.contains("Acceptance Criteria"))
        );
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == "spec/no-code-example")
        );
    }

    /// Empty body yields violations, not a panic.
    #[test]
    fn test_lint_issue_empty_body() {
        // Act
        let result = lint_issue("", Some(&feature_spec()));

        // Assert
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 5);
    }

    /// Generic mode: each of the four checks detects independently.
    #[test]
    fn test_generic_mode_checks() {
        // All checks fire on an empty body.
        let empty = lint_issue("", None);
        assert_eq!(empty.violations.len(), 4);
        assert_eq!(
            empty
                .violations
                .iter()
                .map(|v| v.rule.as_str())
                .collect::<Vec<_>>(),
            vec![
                "generic/body-too-short",
                "generic/acceptance-criteria",
                "generic/no-code-example",
                "generic/no-external-link",
            ]
        );

        // A conforming body passes all four.
        let good = [
            "## Acceptance Criteria",
            "- [ ] first",
            "- [ ] second",
            "Implemented like crates/aptu-core/src/lib.rs per https://example.com/spec (#1702).",
        ]
        .join("\n");
        assert!(
            lint_issue(&good, None).passed,
            "violations: {:?}",
            lint_issue(&good, None).violations
        );
    }

    /// Generic mode: checkboxes hidden in HTML comments or fenced blocks do
    /// not satisfy the acceptance-criteria check; a real fence still counts
    /// as a code example.
    #[test]
    fn test_generic_mode_ignores_hidden_checkboxes() {
        // Arrange: the only checkboxes live in an HTML comment inside the
        // acceptance section, and a fenced block contains two more.
        let body = [
            "## Acceptance Criteria",
            "- [ ] real",
            "- [ ] also real",
            "<!--",
            "- [ ] hidden in comment",
            "- [ ] also hidden",
            "-->",
            "```",
            "- [ ] hidden in fence",
            "- [ ] also fenced",
            "```",
            "Implemented like crates/aptu-core/src/lib.rs per https://example.com/spec (#1702).",
        ]
        .join("\n");

        // Act
        let comment_only = [
            "## Acceptance Criteria",
            "<!--",
            "- [ ] hidden in comment",
            "- [ ] also hidden",
            "-->",
            "Implemented like crates/aptu-core/src/lib.rs per https://example.com/spec (#1702).",
        ]
        .join("\n");
        let result = lint_issue(&body, None);
        let result_comment = lint_issue(&comment_only, None);

        // Assert: the fenced body passes (one real checkbox plus fence for
        // code example); the comment-only body fails acceptance-criteria.
        assert!(result.passed, "violations: {:?}", result.violations);
        assert!(
            result_comment
                .violations
                .iter()
                .any(|v| v.rule == "generic/acceptance-criteria")
        );
    }

    /// TOML parsing accepts the documented `[[spec]]` schema.
    #[test]
    fn test_parse_specs_toml() {
        // Arrange
        let toml_text = r#"
[[spec]]
type = "feature"
required_headings = ["Summary", "Context"]
require_code_examples = true

[[spec]]
type = "bug"
required_headings = ["Summary"]
"#;

        // Act
        let specs = parse_specs(toml_text).expect("valid TOML");

        // Assert
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].issue_type, "feature");
        assert!(find_spec(&specs, "bug").is_some());
        assert!(find_spec(&specs, "docs").is_none());
    }

    /// Broken TOML is an error; unmatched type detection is via `find_spec`.
    #[test]
    fn test_parse_specs_broken_toml() {
        // Act
        let result = parse_specs("this is not toml [[[");

        // Assert
        assert!(result.is_err());
    }

    /// Resolution precedence: explicit config beats repo root beats generic.
    #[test]
    fn test_resolve_specs_precedence() {
        // Arrange: base dir with a repo-root spec and an explicit config.
        let dir = std::env::temp_dir().join("aptu-lint-resolve-test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join(SPECS_FILE_NAME), "[[spec]]\ntype = \"bug\"\n")
            .expect("write repo root");
        let explicit = dir.join("explicit.toml");
        std::fs::write(&explicit, "[[spec]]\ntype = \"feature\"\n").expect("write explicit");

        // Act / Assert: generic when nothing found.
        let empty_dir = std::env::temp_dir().join("aptu-lint-resolve-empty");
        std::fs::create_dir_all(&empty_dir).expect("mkdir");
        assert_eq!(
            resolve_specs_in(&empty_dir, None).expect("generic resolution"),
            SpecResolution::Generic
        );

        // Repo root is used when no explicit config is given.
        assert!(matches!(
            resolve_specs_in(&dir, None).expect("repo-root resolution"),
            SpecResolution::RepoRoot(specs) if specs.len() == 1 && specs[0].issue_type == "bug"
        ));

        // Explicit config wins over the repo-root file.
        assert!(matches!(
            resolve_specs_in(&dir, Some(&explicit)).expect("explicit resolution"),
            SpecResolution::Explicit(specs) if specs.len() == 1 && specs[0].issue_type == "feature"
        ));

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&empty_dir).ok();
    }

    /// Repo-root discovery walks up from a nested directory to the `.git`
    /// ancestor and finds its spec file.
    #[test]
    fn test_find_repo_root_walks_up_to_git_dir() {
        // Arrange: repo root with .git dir and a spec, plus a nested subdir.
        let root = std::env::temp_dir().join("aptu-lint-root-test");
        let nested = root.join("crates").join("nested");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        std::fs::write(root.join(SPECS_FILE_NAME), "[[spec]]\ntype = \"bug\"\n")
            .expect("write spec");

        // Act
        let discovered = find_repo_root(&nested);
        let resolution = resolve_specs_in(&nested, None).expect("resolution");

        // Assert
        assert_eq!(discovered, root);
        assert!(matches!(
            resolution,
            SpecResolution::RepoRoot(specs) if specs.len() == 1 && specs[0].issue_type == "bug"
        ));

        std::fs::remove_dir_all(&root).ok();
    }

    /// Without a `.git` ancestor, discovery falls back to the start dir.
    #[test]
    fn test_find_repo_root_falls_back_to_start() {
        // Arrange
        let plain = std::env::temp_dir().join("aptu-lint-no-git-test");
        std::fs::create_dir_all(&plain).expect("mkdir");

        // Act / Assert
        assert_eq!(find_repo_root(&plain), plain);
        assert_eq!(
            resolve_specs_in(&plain, None).expect("resolution"),
            SpecResolution::Generic
        );

        std::fs::remove_dir_all(&plain).ok();
    }

    /// Malformed explicit config is an error (CLI maps this to exit 2).
    /// External links inside HTML comments or fenced blocks do not satisfy
    /// require_external_link; a visible link does.
    #[test]
    fn test_lint_issue_external_link_ignores_hidden_text() {
        // Arrange
        let body = [
            "## Summary",
            "Something.",
            "## Context",
            "Some context.",
            "## Acceptance Criteria",
            "- [ ] done",
            "<!-- hidden https://example.com and #123 -->",
            "```text",
            "https://in-fence.example.com",
            "```",
            "",
        ]
        .join("\n");

        // Act
        let result = lint_issue(&body, Some(&feature_spec()));

        // Assert
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == "spec/no-external-link"),
            "hidden link should not satisfy require_external_link: {:?}",
            result.violations
        );

        // Arrange
        let body = [
            "## Summary",
            "Something.",
            "## Context",
            "Some context.",
            "## Acceptance Criteria",
            "- [ ] done",
            "<!-- hidden https://hidden.example.com -->",
            "Visible reference: https://example.com and #456.",
            "",
        ]
        .join("\n");

        // Act
        let result = lint_issue(&body, Some(&feature_spec()));

        // Assert
        assert!(
            !result
                .violations
                .iter()
                .any(|v| v.rule == "spec/no-external-link"),
            "visible link should satisfy require_external_link: {:?}",
            result.violations
        );
    }

    #[test]
    fn test_resolve_specs_broken_explicit_config_errors() {
        // Arrange
        let dir = std::env::temp_dir().join("aptu-lint-broken-test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let broken = dir.join("broken.toml");
        std::fs::write(&broken, "not [[ valid toml").expect("write broken");

        // Act
        let result = resolve_specs(Some(&broken));

        // Assert
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
