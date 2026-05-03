// SPDX-License-Identifier: Apache-2.0

//! `scan-security` subcommand: scan a local file or directory for security issues.

use std::path::PathBuf;

use anyhow::Result;
use aptu_core::{AppConfig, Finding, PatternEngine, SarifReport, SecurityScanner};
use walkdir::WalkDir;

use crate::cli::OutputFormat;

/// Run the `scan-security` subcommand.
///
/// Walks `path` (file or directory), applies the embedded pattern engine to each file,
/// collects findings, emits output in the requested format, and exits with code 1 if any
/// finding severity is listed in `fail_on`.
#[allow(clippy::unused_async)]
pub async fn run_scan_security_command(
    path: PathBuf,
    fail_on: Vec<String>,
    exclude: Vec<String>,
    output_format: OutputFormat,
    _config: &AppConfig,
) -> Result<()> {
    let scanner = SecurityScanner::default();
    let mut findings: Vec<Finding> = Vec::new();

    for entry in WalkDir::new(&path)
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

    emit_output(output_format, &findings)?;

    // Exit 1 if any finding severity matches --fail-on list
    if !fail_on.is_empty() {
        let fail_severities: Vec<String> = fail_on.iter().map(|s| s.to_lowercase()).collect();

        let should_fail = findings.iter().any(|f| {
            let sev = format!("{:?}", f.severity).to_lowercase();
            fail_severities.contains(&sev)
        });

        if should_fail {
            std::process::exit(1);
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
