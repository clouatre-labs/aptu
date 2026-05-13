// SPDX-License-Identifier: Apache-2.0

//! `scan-security` subcommand: scan a local file or directory for security issues.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use aptu_core::{AppConfig, Finding, PatternEngine, SarifReport, SecurityScanner};
use walkdir::WalkDir;

use crate::cli::OutputFormat;

/// Maximum allowed size for a diff input (5 MiB).
const DIFF_SIZE_LIMIT: usize = 5_242_880;

/// Run the `scan-security` subcommand.
///
/// When `diff` is provided, reads a unified diff from a file path or stdin (`-`),
/// enforces a 5 MiB size limit, and calls `scanner.scan_diff()`.
/// When `path` is provided, walks the file or directory and calls `scanner.scan_file()`.
#[allow(clippy::unused_async)]
pub async fn run_scan_security_command(
    path: Option<PathBuf>,
    diff: Option<PathBuf>,
    fail_on: Vec<String>,
    exclude: Vec<String>,
    output_format: OutputFormat,
    _config: &AppConfig,
) -> Result<()> {
    let scanner = SecurityScanner::default();
    let mut findings: Vec<Finding> = Vec::new();

    if let Some(diff_path) = diff {
        // Diff mode: read from file or stdin
        let content = if diff_path == Path::new("-") {
            let mut buf = String::new();
            std::io::stdin()
                .take((DIFF_SIZE_LIMIT + 1) as u64)
                .read_to_string(&mut buf)
                .map_err(|e| anyhow::anyhow!("Failed to read stdin: {e}"))?;
            buf
        } else {
            let meta = std::fs::metadata(&diff_path)
                .map_err(|e| anyhow::anyhow!("Cannot stat '{}': {e}", diff_path.display()))?;
            if meta.len() > DIFF_SIZE_LIMIT as u64 {
                return Err(anyhow::anyhow!(
                    "Diff file '{}' exceeds the 5 MiB limit ({} bytes)",
                    diff_path.display(),
                    meta.len()
                ));
            }
            std::fs::read_to_string(&diff_path)
                .map_err(|e| anyhow::anyhow!("Failed to read '{}': {e}", diff_path.display()))?
        };

        if content.len() > DIFF_SIZE_LIMIT {
            return Err(anyhow::anyhow!(
                "Diff input exceeds the 5 MiB limit ({} bytes)",
                content.len()
            ));
        }

        findings.extend(scanner.scan_diff(&content));
    } else {
        // Walk mode: path is guaranteed present by Clap (required_unless_present = "diff")
        let scan_path = path.expect("path required when --diff is not provided");

        for entry in WalkDir::new(&scan_path)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if entry.file_type().is_dir() {
                continue;
            }

            let file_path = entry.path();
            let file_path_str = file_path.to_string_lossy();

            // Apply --exclude prefix filter
            if exclude
                .iter()
                .any(|prefix| file_path_str.starts_with(prefix.as_str()))
            {
                continue;
            }

            // Read file content; skip files that cannot be read (binary, permission denied, etc.)
            let Ok(content) = std::fs::read_to_string(file_path) else {
                continue;
            };

            let file_findings = scanner.scan_file(&content, &file_path_str);
            findings.extend(file_findings);
        }
    }

    emit_output(output_format, &findings)?;

    // Exit 1 if any finding severity matches --fail-on list
    if !fail_on.is_empty() {
        let fail_severities: Vec<String> = fail_on.iter().map(|s| s.to_lowercase()).collect();

        let should_fail = findings
            .iter()
            .any(|f| fail_severities.iter().any(|s| s == f.severity.as_str()));

        if should_fail {
            return Err(anyhow::Error::new(crate::errors::ScanFindingsExit));
        }
    }

    Ok(())
}

/// Emit findings in the requested output format.
fn emit_output(output_format: OutputFormat, findings: &[Finding]) -> Result<()> {
    match output_format {
        OutputFormat::Sarif => {
            let engine = PatternEngine::from_embedded_json()?;
            let patterns = engine.definitions();
            let report = SarifReport::with_rules(findings.to_vec(), &patterns);
            let json = serde_json::to_string_pretty(&report)
                .map_err(|e| anyhow::anyhow!("Failed to serialize SARIF: {e}"))?;
            println!("{json}");
        }
        OutputFormat::GithubAnnotations => {
            for f in findings {
                println!(
                    "::error file={},line={},title={}::{}",
                    f.file_path, f.line_number, f.pattern_id, f.description
                );
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(findings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize findings to JSON: {e}"))?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_saphyr::to_string(&findings.to_vec())
                .map_err(|e| anyhow::anyhow!("Failed to serialize findings to YAML: {e}"))?;
            println!("{yaml}");
        }
        OutputFormat::Text | OutputFormat::Markdown => {
            if findings.is_empty() {
                println!("No security findings.");
            } else {
                println!("Security findings ({}):", findings.len());
                for f in findings {
                    println!(
                        "  [{}] {} ({}:{}): {}",
                        format!("{:?}", f.severity).to_uppercase(),
                        f.pattern_id,
                        f.file_path,
                        f.line_number,
                        f.description
                    );
                }
            }
        }
    }
    Ok(())
}
