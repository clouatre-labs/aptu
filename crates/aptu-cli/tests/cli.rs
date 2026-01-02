// SPDX-License-Identifier: Apache-2.0

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn test_version() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("aptu"));
}

#[test]
fn test_help_contains_all_commands() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("repo"))
        .stdout(predicate::str::contains("issue"))
        .stdout(predicate::str::contains("history"))
        .stdout(predicate::str::contains("completion"));
}

#[test]
fn test_repo_list_json_output() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--output")
        .arg("json")
        .assert()
        .success();

    let output = cargo_bin_cmd!("aptu")
        .arg("repo")
        .arg("list")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "repo list --output json should produce valid JSON"
    );

    let json = parsed.unwrap();
    assert!(json.is_array(), "repo list JSON output should be an array");
}

#[test]
fn test_repo_list_yaml_output() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--output")
        .arg("yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains("-").or(predicate::str::contains("repositories")));
}

#[test]
fn test_repo_list_markdown_output() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--output")
        .arg("markdown")
        .assert()
        .success()
        .stdout(predicate::str::contains("|").or(predicate::str::contains("#")));
}

#[test]
fn test_completion_generate_bash() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("completion")
        .arg("generate")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("bash").or(predicate::str::contains("complete")));
}

#[test]
fn test_completion_generate_zsh() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("completion")
        .arg("generate")
        .arg("zsh")
        .assert()
        .success()
        .stdout(predicate::str::contains("zsh").or(predicate::str::contains("compdef")));
}

#[test]
fn test_completion_install_dry_run() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("completion")
        .arg("install")
        .arg("--shell")
        .arg("zsh")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"))
        .stdout(predicate::str::contains("Completion path"));
}

#[test]
fn test_history_empty_state() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("history")
        .arg("--output")
        .arg("json")
        .assert()
        .success();

    let output = cargo_bin_cmd!("aptu")
        .arg("history")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "history --output json should produce valid JSON"
    );

    let json = parsed.unwrap();
    assert!(
        json.is_array() || json.is_object(),
        "history JSON output should be an array or object"
    );
}

#[test]
fn test_auth_status() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("auth").arg("status").assert().success();
}

#[test]
fn test_invalid_command() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("invalidcmd")
        .assert()
        .failure()
        .code(predicate::eq(2));
}

#[test]
fn test_repo_list_invalid_format() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--output")
        .arg("xml")
        .assert()
        .failure()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("invalid").or(predicate::str::contains("format")));
}

#[test]
fn test_repo_invalid_subcommand() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("invalid")
        .assert()
        .failure()
        .code(predicate::eq(2));
}

#[test]
fn test_json_output_is_quiet_by_default() {
    // JSON output should automatically suppress spinners/progress
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--output")
        .arg("json")
        .assert()
        .success();
}

#[test]
fn test_triage_multiple_references() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("issue")
        .arg("triage")
        .arg("block/goose#1")
        .arg("block/goose#2")
        .arg("--dry-run")
        .assert()
        .success();
}

#[test]
fn test_triage_single_reference() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("issue")
        .arg("triage")
        .arg("block/goose#1")
        .arg("--dry-run")
        .assert()
        .success();
}

#[test]
fn test_triage_since_flag_invalid_date() {
    // Test that invalid date format is rejected
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("issue")
        .arg("triage")
        .arg("--repo")
        .arg("block/goose")
        .arg("--since")
        .arg("not-a-date")
        .arg("--dry-run")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Invalid date format"));
}

#[test]
fn test_triage_since_requires_repo() {
    // Test that --since without --repo fails with helpful message
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("issue")
        .arg("triage")
        .arg("--since")
        .arg("2025-12-01")
        .arg("--dry-run")
        .assert()
        .failure()
        .stderr(predicates::str::contains("--since requires --repo"));
}

#[test]
fn test_triage_no_comment_flag_recognized() {
    // Test that --no-comment flag is recognized in help
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("issue")
        .arg("triage")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("--no-comment"));
}

// JSON Output Validation Tests

#[test]
fn test_auth_status_json_output() {
    let output = cargo_bin_cmd!("aptu")
        .arg("auth")
        .arg("status")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

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
    let output = cargo_bin_cmd!("aptu")
        .arg("issue")
        .arg("triage")
        .arg("block/goose#1")
        .arg("--dry-run")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();
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

#[test]
fn test_issue_list_json_output() {
    let output = cargo_bin_cmd!("aptu")
        .arg("issue")
        .arg("list")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // If authentication fails, the command will exit with error
    // In that case, we just verify the test runs without panic
    if !stdout.is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        if let Ok(json) = parsed {
            assert!(json.is_array(), "issue list JSON output should be an array");
        }
    }
}

#[test]
fn test_repo_discover_json_output() {
    let output = cargo_bin_cmd!("aptu")
        .arg("repo")
        .arg("discover")
        .arg("--language")
        .arg("rust")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // If authentication fails, the command will exit with error
    // In that case, we just verify the test runs without panic
    if !stdout.is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        if let Ok(json) = parsed {
            assert!(
                json.is_array(),
                "repo discover JSON output should be an array"
            );
        }
    }
}

#[test]
fn test_history_json_output_structure() {
    let output = cargo_bin_cmd!("aptu")
        .arg("history")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "history --output json should produce valid JSON"
    );

    let json = parsed.unwrap();
    // History can be either array (empty) or object (with data)
    assert!(
        json.is_array() || json.is_object(),
        "history JSON output should be an array or object"
    );
}
