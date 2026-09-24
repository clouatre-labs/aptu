// SPDX-License-Identifier: Apache-2.0

//! `lint-issue` subcommand: deterministic issue-body validation.

use std::path::PathBuf;

use anyhow::Result;
use aptu_core::AppConfig;
use aptu_core::issue_lint::{IssueLintSpec, find_spec, lint_issue, resolve_specs};

use crate::cli::OutputFormat;
use crate::commands::workflow::{escape_workflow_data, escape_workflow_property};
use crate::errors::LintConfigErrorExit;
use crate::errors::LintViolationsExit;

/// Maximum allowed size for an issue body file (5 MiB).
const BODY_SIZE_LIMIT: usize = 5_242_880;

/// Run the `lint-issue` subcommand.
///
/// Reads the issue body from `file`, resolves specs (explicit `--config` >
/// repo-root `issue-lint-specs.toml` > generic mode), emits the lint result
/// in the requested output format, and returns a `LintViolationsExit`
/// sentinel when violations exist. A broken or unmatched explicit config
/// returns `LintConfigErrorExit` after printing a clear error to stderr.
#[allow(clippy::unused_async)]
pub async fn run_lint_issue_command(
    file: PathBuf,
    issue_type: Option<String>,
    config: Option<PathBuf>,
    output_format: OutputFormat,
    _app_config: &AppConfig,
) -> Result<()> {
    // SARIF is a security-scanning format and is not supported for lint-issue.
    if matches!(output_format, OutputFormat::Sarif) {
        return Err(anyhow::anyhow!(
            "output format 'sarif' is not supported for lint-issue (use text, json, or github-annotations)"
        ));
    }

    let body = read_body(&file)?;

    // Resolve specs; broken configs print a clear error, then exit 2.
    let resolution = resolve_specs(config.as_deref());
    let spec: Option<IssueLintSpec> = match resolution {
        Err(err) => {
            eprintln!("Error: {err:#}");
            return Err(anyhow::Error::new(LintConfigErrorExit));
        }
        Ok(aptu_core::issue_lint::SpecResolution::Explicit(specs)) => {
            // Spec matching requires a type; an explicit config without
            // --issue-type is a configuration error (exit 2).
            let Some(issue_type) = issue_type.as_deref() else {
                eprintln!("Error: --issue-type is required when an explicit config is supplied");
                return Err(anyhow::Error::new(LintConfigErrorExit));
            };
            if let Some(spec) = find_spec(&specs, issue_type) {
                Some(spec.clone())
            } else {
                eprintln!(
                    "Error: explicit config {} has no [[spec]] with type \"{}\" (available: {})",
                    config
                        .as_deref()
                        .unwrap_or(std::path::Path::new("-"))
                        .display(),
                    issue_type,
                    spec_type_list(&specs)
                );
                return Err(anyhow::Error::new(LintConfigErrorExit));
            }
        }
        Ok(aptu_core::issue_lint::SpecResolution::RepoRoot(specs)) => {
            let matched = issue_type
                .as_deref()
                .and_then(|issue_type| find_spec(&specs, issue_type))
                .cloned();
            // A repo-root spec that lacks the type falls back to generic.
            matched
        }
        Ok(aptu_core::issue_lint::SpecResolution::Generic) => None,
    };

    let result = lint_issue(&body, spec.as_ref());

    emit_output(output_format, &result)?;

    if !result.passed {
        return Err(anyhow::Error::new(LintViolationsExit));
    }
    Ok(())
}

/// Reads the issue body file with a size limit.
fn read_body(file: &std::path::Path) -> Result<String> {
    let meta = std::fs::metadata(file)
        .map_err(|e| anyhow::anyhow!("Cannot read '{}': {e}", file.display()))?;
    if meta.len() > BODY_SIZE_LIMIT as u64 {
        return Err(anyhow::anyhow!(
            "Issue body file '{}' exceeds the 5 MiB limit ({} bytes)",
            file.display(),
            meta.len()
        ));
    }
    std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read '{}': {e}", file.display()))
}

/// Formats the spec type list for error messages.
fn spec_type_list(specs: &[IssueLintSpec]) -> String {
    specs
        .iter()
        .map(|s| s.issue_type.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Emit the lint result in the requested output format.
fn emit_output(
    output_format: OutputFormat,
    result: &aptu_core::issue_lint::IssueLintResult,
) -> Result<()> {
    match output_format {
        OutputFormat::GithubAnnotations => {
            for v in &result.violations {
                println!(
                    "::error line={},title={}::{}",
                    v.line.unwrap_or(1),
                    escape_workflow_property(&v.rule),
                    escape_workflow_data(&v.message)
                );
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(result)
                .map_err(|e| anyhow::anyhow!("Failed to serialize lint result to JSON: {e}"))?;
            println!("{json}");
        }
        // Unreachable: SARIF is rejected before the lint run.
        OutputFormat::Sarif => unreachable!("sarif output is rejected before linting"),
        OutputFormat::Text => {
            if result.passed {
                println!("Issue body passed lint.");
            } else {
                println!("Issue lint violations ({}):", result.violations.len());
                for v in &result.violations {
                    match v.line {
                        Some(line) => println!("  [{}] (line {line}) {}", v.rule, v.message),
                        None => println!("  [{}] {}", v.rule, v.message),
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::LintConfigErrorExit;
    use crate::errors::LintViolationsExit;

    const CONFORMING_BODY: &str = "\
## Summary
Add a deterministic lint operation for issue bodies with enough detail to act on.
## Context
Follows the scan-security pattern described in docs/SECURITY_SCANNING.md (#1702).
## Implementation Notes
Mirror the security module; see crates/aptu-core/src/lib.rs.
## Acceptance Criteria
- [ ] passes on conforming bodies
- [ ] fails with named headings
## Not In Scope
App-managed mode.
";

    fn write_temp(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, content).expect("write temp file");
        path
    }

    fn cleanup(path: &std::path::Path) {
        std::fs::remove_file(path).ok();
    }

    /// Violations path emits output, then exits 1 silently (sentinel).
    #[test]
    fn test_violations_return_lint_violations_sentinel() {
        // Arrange: body missing every required heading.
        let file = write_temp("aptu-lint-violations-body.md", "one-liner\n");
        let config = write_temp(
            "aptu-lint-violations-specs.toml",
            "[[spec]]\ntype = \"feature\"\nrequired_headings = [\"Summary\"]\n",
        );

        // Act
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(run_lint_issue_command(
            file.clone(),
            Some("feature".to_string()),
            Some(config.clone()),
            OutputFormat::Text,
            &AppConfig::default(),
        ));

        // Assert: sentinel error type, output already emitted to stdout.
        let err = result.expect_err("expected violations sentinel");
        assert!(err.root_cause().is::<LintViolationsExit>());
        cleanup(&file);
        cleanup(&config);
    }

    /// Unknown type with an explicit unmatched config exits 2.
    #[test]
    fn test_unknown_type_with_explicit_config_is_config_error() {
        // Arrange
        let file = write_temp("aptu-lint-unmatched-body.md", CONFORMING_BODY);
        let config = write_temp(
            "aptu-lint-unmatched-specs.toml",
            "[[spec]]\ntype = \"feature\"\nrequired_headings = [\"Summary\"]\n",
        );

        // Act
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(run_lint_issue_command(
            file.clone(),
            Some("nonexistent".to_string()),
            Some(config.clone()),
            OutputFormat::Text,
            &AppConfig::default(),
        ));

        // Assert
        let err = result.expect_err("expected config error sentinel");
        assert!(err.root_cause().is::<LintConfigErrorExit>());
        cleanup(&file);
        cleanup(&config);
    }

    /// Broken explicit TOML config exits 2.
    #[test]
    fn test_broken_config_is_config_error() {
        // Arrange
        let file = write_temp("aptu-lint-broken-body.md", CONFORMING_BODY);
        let config = write_temp("aptu-lint-broken-specs.toml", "not [[ valid toml");

        // Act
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(run_lint_issue_command(
            file.clone(),
            Some("feature".to_string()),
            Some(config.clone()),
            OutputFormat::Text,
            &AppConfig::default(),
        ));

        // Assert
        let err = result.expect_err("expected config error sentinel");
        assert!(err.root_cause().is::<LintConfigErrorExit>());
        cleanup(&file);
        cleanup(&config);
    }

    /// A conforming body against an explicit spec passes (Ok).
    #[test]
    fn test_conforming_body_passes() {
        // Arrange
        let file = write_temp("aptu-lint-conforming-body.md", CONFORMING_BODY);
        let config = write_temp(
            "aptu-lint-conforming-specs.toml",
            "[[spec]]\ntype = \"feature\"\nrequired_headings = [\"Summary\", \"Context\"]\n",
        );

        // Act
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(run_lint_issue_command(
            file.clone(),
            Some("feature".to_string()),
            Some(config.clone()),
            OutputFormat::Text,
            &AppConfig::default(),
        ));

        // Assert
        assert!(result.is_ok(), "unexpected error: {result:?}");
        cleanup(&file);
        cleanup(&config);
    }

    /// Sarif output is rejected with a clear error for lint-issue.
    #[test]
    fn test_sarif_output_is_rejected() {
        // Arrange
        let file = write_temp("aptu-lint-sarif-body.md", CONFORMING_BODY);

        // Act
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(run_lint_issue_command(
            file.clone(),
            Some("feature".to_string()),
            None,
            OutputFormat::Sarif,
            &AppConfig::default(),
        ));

        // Assert
        let err = result.expect_err("expected sarif rejection");
        assert!(err.to_string().contains("sarif"));
        cleanup(&file);
    }
}
