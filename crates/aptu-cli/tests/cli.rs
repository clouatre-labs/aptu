// SPDX-License-Identifier: Apache-2.0

use assert_cmd::cargo::cargo_bin_cmd;

/// Run the aptu binary with the given arguments and return its output.
fn run_cli(args: &[&str]) -> std::process::Output {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.args(args).output().unwrap()
}

/// Run the aptu binary with the given arguments and stdin, returning its output.
fn run_cli_with_stdin(args: &[&str], stdin: &str) -> std::process::Output {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.args(args).write_stdin(stdin).output().unwrap()
}

#[test]
fn test_version() {
    let output = run_cli(&["--version"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("aptu"));
}

#[test]
fn test_help_contains_all_commands() {
    let output = run_cli(&["--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("auth"));
    assert!(stdout.contains("issue"));
    assert!(stdout.contains("completion"));
}

#[test]
fn test_completion_generate_bash() {
    let output = run_cli(&["completion", "generate", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bash") || stdout.contains("complete"));
}

#[test]
fn test_completion_generate_zsh() {
    let output = run_cli(&["completion", "generate", "zsh"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("zsh") || stdout.contains("compdef"));
}

#[test]
fn test_completion_install_dry_run() {
    let output = run_cli(&["completion", "install", "--shell", "zsh", "--dry-run"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DRY RUN"));
    assert!(stdout.contains("Completion path"));
}

#[test]
fn test_invalid_command() {
    let output = run_cli(&["invalidcmd"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn test_triage_multiple_references() {
    let output = run_cli(&[
        "issue",
        "triage",
        "block/goose#1",
        "block/goose#2",
        "--dry-run",
    ]);
    assert!(output.status.success());
}

#[test]
fn test_triage_single_reference() {
    let output = run_cli(&["issue", "triage", "block/goose#1", "--dry-run"]);
    assert!(output.status.success());
}

#[test]
fn test_triage_since_flag_invalid_date() {
    // Test that invalid date format is rejected
    let output = run_cli(&[
        "issue",
        "triage",
        "--repo",
        "block/goose",
        "--since",
        "not-a-date",
        "--dry-run",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid date format"));
}

#[test]
fn test_triage_since_requires_repo() {
    // Test that --since without explicit --repo works due to auto-inference.
    // When running in a git repository (like the aptu repo itself), the repo
    // is automatically inferred. The command may fail with auth error in CI
    // (no token), but it should NOT fail with "--since requires --repo".
    // This proves auto-inference is working.
    let output = run_cli(&["issue", "triage", "--since", "2025-12-01", "--dry-run"]);

    // Either succeeds (local with auth) or fails with auth error (CI without auth)
    // but never with "--since requires --repo" (that would mean inference failed)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("--since requires --repo"),
        "Auto-inference should have found repo from git remote"
    );
}

#[test]
fn test_triage_no_comment_flag_recognized() {
    // Test that --no-comment flag is recognized in help
    let output = run_cli(&["issue", "triage", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--no-comment"));
}

// JSON Output Validation Tests

#[test]
fn test_auth_status_json_output() {
    let output = run_cli(&["auth", "status", "--output", "json"]);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "auth status --output json should produce valid JSON"
    );

    let json = parsed.unwrap();
    assert!(
        json.is_object(),
        "auth status JSON output should be an object"
    );
    assert!(
        json.get("authenticated").is_some(),
        "auth status JSON should have 'authenticated' field"
    );
}

#[test]
fn test_issue_triage_dry_run_json_output() {
    // Note: This test requires valid GitHub authentication
    // It will be skipped if not authenticated, but validates JSON output when it runs
    let output = run_cli(&[
        "issue",
        "triage",
        "block/goose#1",
        "--dry-run",
        "--output",
        "json",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    // If authentication fails, the command will exit with error
    // In that case, we just verify the test runs without panic
    if !stdout.is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        if let Ok(json) = parsed {
            assert!(
                json.is_object(),
                "issue triage JSON output should be an object"
            );
            assert!(
                json.get("issue_number").is_some(),
                "issue triage JSON should have 'issue_number' field"
            );
            assert!(
                json.get("triage").is_some(),
                "issue triage JSON should have 'triage' field"
            );
            assert!(
                json.get("dry_run").is_some(),
                "issue triage JSON should have 'dry_run' field"
            );
        }
    }
}

// --- scan-security --diff integration tests ---

#[test]
fn scan_security_diff_file_json() {
    use std::io::Write;
    // Arrange: write a temp file with a unified diff containing a hardcoded API key pattern
    let diff_content = concat!(
        "diff --git a/config.py b/config.py\n",
        "--- a/config.py\n",
        "+++ b/config.py\n",
        "@@ -1,2 +1,3 @@\n",
        " # config\n",
        "+api_key = \"abcdefghij1234567890xyz\"\n",
        " pass\n"
    );
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{diff_content}").unwrap();

    // Act
    let output = run_cli(&[
        "scan-security",
        "--diff",
        tmp.path().to_str().unwrap(),
        "--output",
        "json",
    ]);

    // Assert: exit 0 (no --fail-on) and findings array is non-empty
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert!(parsed.is_array(), "expected JSON array of findings");
    assert!(
        !parsed.as_array().unwrap().is_empty(),
        "expected at least one finding for hardcoded API key"
    );
}

#[test]
fn scan_security_diff_stdin() {
    // Arrange: same diff piped via stdin using sentinel -
    let diff_content = concat!(
        "diff --git a/config.py b/config.py\n",
        "--- a/config.py\n",
        "+++ b/config.py\n",
        "@@ -1,2 +1,3 @@\n",
        " # config\n",
        "+api_key = \"abcdefghij1234567890xyz\"\n",
        " pass\n"
    );

    // Act
    let output = run_cli_with_stdin(
        &["scan-security", "--diff", "-", "--output", "json"],
        diff_content,
    );

    // Assert: exit 0 and non-empty findings
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert!(parsed.is_array(), "expected JSON array of findings");
    assert!(
        !parsed.as_array().unwrap().is_empty(),
        "expected at least one finding for hardcoded API key via stdin"
    );
}

#[test]
fn scan_security_diff_oversize_error() {
    use std::io::Write;

    // Arrange: write a file larger than 5 MiB
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let chunk = b"x".repeat(1024);
    for _ in 0..=(5 * 1024) {
        tmp.write_all(&chunk).unwrap();
    }
    tmp.flush().unwrap();

    // Act
    let output = run_cli(&["scan-security", "--diff", tmp.path().to_str().unwrap()]);

    // Assert: non-zero exit due to size limit
    assert!(
        !output.status.success(),
        "expected non-zero exit for oversized diff"
    );
}

#[test]
fn scan_security_conflicts_path_and_diff() {
    // Arrange: create a temp file for --diff
    let tmp = tempfile::NamedTempFile::new().unwrap();

    // Act: pass both a path and --diff; Clap should reject
    let output = run_cli(&["scan-security", ".", "--diff", tmp.path().to_str().unwrap()]);

    // Assert: non-zero exit (Clap argument conflict error)
    assert!(
        !output.status.success(),
        "expected non-zero exit when both path and --diff are provided"
    );
}

#[test]
fn scan_security_sarif_output_writes_valid_sarif() {
    use std::io::Write;

    // Arrange: write a temp file with a unified diff containing a hardcoded API key pattern
    let diff_content = concat!(
        "diff --git a/config.py b/config.py\n",
        "--- a/config.py\n",
        "+++ b/config.py\n",
        "@@ -1,2 +1,3 @@\n",
        " # config\n",
        "+api_key = \"abcdefghij1234567890xyz\"\n",
        " pass\n"
    );
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{diff_content}").unwrap();

    let sarif_output = tempfile::NamedTempFile::new().unwrap();

    // Act: run with --output github-annotations and --sarif-output
    let output = run_cli(&[
        "scan-security",
        "--diff",
        tmp.path().to_str().unwrap(),
        "--output",
        "github-annotations",
        "--sarif-output",
        sarif_output.path().to_str().unwrap(),
    ]);

    // Assert: exit 0
    assert!(output.status.success(), "expected exit 0");

    // Assert: SARIF file contains valid SARIF 2.1.0 JSON
    let sarif_content = std::fs::read_to_string(sarif_output.path()).unwrap();
    let sarif_parsed: serde_json::Value =
        serde_json::from_str(&sarif_content).expect("SARIF output must be valid JSON");
    assert_eq!(sarif_parsed["version"], "2.1.0");
    assert!(
        sarif_parsed["runs"].is_array(),
        "expected runs array in SARIF output"
    );
}

#[test]
fn scan_security_sarif_output_written_before_fail_on_exit() {
    use std::io::Write;

    // Arrange: write a diff with a finding that will trigger --fail-on
    let diff_content = concat!(
        "diff --git a/config.py b/config.py\n",
        "--- a/config.py\n",
        "+++ b/config.py\n",
        "@@ -1,2 +1,3 @@\n",
        " # config\n",
        "+api_key = \"abcdefghij1234567890xyz\"\n",
        " pass\n"
    );
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{diff_content}").unwrap();

    let sarif_output = tempfile::NamedTempFile::new().unwrap();

    // Act: run with --sarif-output and --fail-on critical,high
    let output = run_cli(&[
        "scan-security",
        "--diff",
        tmp.path().to_str().unwrap(),
        "--output",
        "github-annotations",
        "--sarif-output",
        sarif_output.path().to_str().unwrap(),
        "--fail-on",
        "critical,high",
    ]);

    // Assert: non-zero exit due to --fail-on
    assert!(
        !output.status.success(),
        "expected non-zero exit due to --fail-on"
    );

    // Assert: SARIF file is written before the non-zero exit
    let sarif_content = std::fs::read_to_string(sarif_output.path()).unwrap();
    let sarif_parsed: serde_json::Value =
        serde_json::from_str(&sarif_content).expect("SARIF must be valid JSON");
    assert_eq!(sarif_parsed["version"], "2.1.0");
    assert!(
        sarif_parsed["runs"][0]["results"]
            .as_array()
            .is_some_and(|r| !r.is_empty()),
        "expected at least one SARIF result"
    );
}

// --- output-format completeness matrix ---

/// Offline-safe commands whose outcome rendering is exercised under every
/// `OutputFormat` variant. Network-dependent commands (issue, pr, models) are
/// excluded to keep the matrix deterministic.
const FORMAT_MATRIX_ARGS: [&[&str]; 2] = [&["auth", "status"], &["scan-security", "--diff", "-"]];

const ALL_FORMATS: [&str; 4] = ["text", "json", "sarif", "github-annotations"];

#[test]
fn output_format_completeness_matrix() {
    // A minimal unified diff; scan-security scans it offline for known patterns.
    let diff_content = concat!(
        "diff --git a/config.py b/config.py\n",
        "--- a/config.py\n",
        "+++ b/config.py\n",
        "@@ -1,2 +1,3 @@\n",
        " # config\n",
        "+api_key = \"abcdefghij1234567890xyz\"\n",
        " pass\n"
    );

    for args in FORMAT_MATRIX_ARGS {
        for format in ALL_FORMATS {
            let mut full_args = args.to_vec();
            full_args.extend(["--output", format]);

            let output = if args.contains(&"-") {
                run_cli_with_stdin(&full_args, diff_content)
            } else {
                run_cli(&full_args)
            };

            assert!(
                output.status.success(),
                "command {args:?} should succeed under --output {format}"
            );

            let stdout = String::from_utf8(output.stdout).unwrap();
            match format {
                "text" | "json" => {
                    assert!(
                        !stdout.trim().is_empty(),
                        "command {args:?} under {format} should produce output"
                    );
                    if format == "json" {
                        serde_json::from_str::<serde_json::Value>(&stdout)
                            .unwrap_or_else(|e| panic!("command {args:?} under json: {e}"));
                    }
                }
                "sarif" => {
                    let sarif: serde_json::Value = serde_json::from_str(&stdout)
                        .unwrap_or_else(|e| panic!("command {args:?} under sarif: {e}"));
                    assert_eq!(sarif["version"], "2.1.0");
                    assert!(
                        sarif["runs"].is_array(),
                        "command {args:?} under sarif should emit valid SARIF with runs"
                    );
                }
                "github-annotations" => {
                    if args[0] == "auth" {
                        // Documented no-op: non-scan commands emit nothing in
                        // this format.
                        assert!(
                            stdout.trim().is_empty(),
                            "auth status under github-annotations should emit nothing"
                        );
                    } else if args[0] == "scan-security" {
                        // Documented contract: one GitHub workflow annotation
                        // command line per finding, empty when there are none.
                        for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
                            assert!(
                                line.starts_with("::error file=")
                                    && line.contains("line=")
                                    && line.contains("title="),
                                "scan-security under github-annotations should emit \
                                 annotation commands, got: {line:?}"
                            );
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}
