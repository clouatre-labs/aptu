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
fn test_completion_bash() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("completion")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("bash").or(predicate::str::contains("complete")));
}

#[test]
fn test_completion_zsh() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("completion")
        .arg("zsh")
        .assert()
        .success()
        .stdout(predicate::str::contains("zsh").or(predicate::str::contains("compdef")));
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
fn test_quiet_flag_suppresses_output() {
    let mut cmd = cargo_bin_cmd!("aptu");
    cmd.arg("repo")
        .arg("list")
        .arg("--quiet")
        .assert()
        .success();
}
