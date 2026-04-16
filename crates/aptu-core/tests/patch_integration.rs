// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `apply_patch_and_push` using a local bare repository as origin.
//!
//! All tests are network-free: a bare git repository is created in a temporary
//! directory and wired up as `origin` for the working repository.

use aptu_core::git::patch::{PatchError, PatchStep, apply_patch_and_push};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// Embed the diff fixture at compile time.
const SIMPLE_DIFF: &str = include_str!("patch_fixtures/simple.diff");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Initialise a working git repository with a single `hello.txt` commit on `main`.
/// Returns (working_dir TempDir, bare_origin TempDir).
fn setup_repo() -> (TempDir, TempDir) {
    let work = TempDir::new().expect("create work tmpdir");
    let bare = TempDir::new().expect("create bare tmpdir");

    // Init bare repo (the origin).
    run(
        &[
            "git",
            "init",
            "--bare",
            bare.path().to_str().expect("bare path"),
        ],
        bare.path(),
    );

    // Init working repo.
    let w = work.path();
    run(
        &["git", "init", "-b", "main", w.to_str().expect("work path")],
        w,
    );
    git(w, &["config", "user.email", "test@example.com"]);
    git(w, &["config", "user.name", "Test User"]);
    git(w, &["config", "commit.gpgSign", "false"]);
    // Disable hooks inherited from global git templates so tests are not
    // affected by repository-local commit hooks (e.g. email allowlists).
    // Use a subdirectory of the work tmpdir rather than /dev/null for
    // cross-platform compatibility.
    let hooks_dir = work.path().join("hooks");
    std::fs::create_dir_all(&hooks_dir).expect("create hooks dir");
    git(
        w,
        &[
            "config",
            "core.hooksPath",
            hooks_dir.to_str().expect("hooks path"),
        ],
    );

    // Wire origin.
    git(
        w,
        &[
            "remote",
            "add",
            "origin",
            bare.path().to_str().expect("bare path"),
        ],
    );

    // Create hello.txt with the content the patch expects to replace.
    std::fs::write(w.join("hello.txt"), "hello world\n").expect("write hello.txt");
    git(w, &["add", "hello.txt"]);
    git(w, &["commit", "-m", "initial commit"]);

    // Push main to origin so apply_patch_and_push can checkout origin/main.
    git(w, &["push", "origin", "main"]);

    (work, bare)
}

/// Run an arbitrary command; panic on failure.
fn run(cmd: &[&str], cwd: &Path) {
    let status = Command::new(cmd[0])
        .args(&cmd[1..])
        .current_dir(cwd)
        .status()
        .expect("spawn command");
    assert!(status.success(), "command failed: {cmd:?}");
}

/// Run a git subcommand in the given directory; panic on failure.
fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in {cwd:?}");
}

/// Write the embedded diff fixture to a temporary file and return its path.
fn write_diff(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("patch.diff");
    std::fs::write(&p, SIMPLE_DIFF).expect("write diff fixture");
    p
}

/// Collect progress steps into a Vec for assertion.
fn collecting_progress() -> (
    std::sync::Arc<std::sync::Mutex<Vec<PatchStep>>>,
    impl Fn(PatchStep),
) {
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let steps_clone = steps.clone();
    let cb = move |s: PatchStep| {
        steps_clone.lock().expect("lock steps").push(s);
    };
    (steps, cb)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Happy path: valid patch is applied, committed, and pushed to the bare remote.
#[tokio::test]
async fn test_apply_patch_happy_path() {
    let (work, _bare) = setup_repo();
    let w = work.path();
    let diff_path = write_diff(w);

    let (steps, progress) = collecting_progress();

    let branch = apply_patch_and_push(
        &diff_path,
        w,
        Some("test/happy-path"),
        "main",
        "test: happy path",
        false,
        false,
        false,
        progress,
    )
    .await
    .expect("apply_patch_and_push should succeed");

    assert_eq!(branch, "test/happy-path");

    // Verify the commit exists on the branch.
    let log = Command::new("git")
        .args(["log", "--oneline", "test/happy-path"])
        .current_dir(w)
        .output()
        .expect("git log");
    let log_str = String::from_utf8_lossy(&log.stdout);
    assert!(
        log_str.contains("test: happy path"),
        "commit not found in log: {log_str}"
    );

    // Verify Pushing step was reached.
    let recorded = steps.lock().expect("lock").clone();
    assert!(
        recorded.contains(&PatchStep::Pushing),
        "expected Pushing step, got: {recorded:?}"
    );
}

/// Dry-run: patch validates successfully but no commit is created.
#[tokio::test]
async fn test_apply_patch_dry_run_no_commits() {
    let (work, _bare) = setup_repo();
    let w = work.path();
    let diff_path = write_diff(w);

    let branch = apply_patch_and_push(
        &diff_path,
        w,
        Some("test/dry-run"),
        "main",
        "test: dry run",
        false,
        false,
        true, // dry_run = true
        |_| {},
    )
    .await
    .expect("dry-run should return branch name");

    assert_eq!(branch, "test/dry-run");

    // No commit should have been created for that branch.
    let result = Command::new("git")
        .args(["rev-parse", "test/dry-run"])
        .current_dir(w)
        .output()
        .expect("git rev-parse");
    assert!(
        !result.status.success(),
        "branch 'test/dry-run' should not exist after dry-run"
    );
}

/// Bad patch: `git apply --check` fails and `PatchError::ApplyCheckFailed` is returned.
#[tokio::test]
async fn test_apply_patch_bad_patch_rejected() {
    let (work, _bare) = setup_repo();
    let w = work.path();

    // Write a patch that targets a file that doesn't match the repo content.
    let bad_diff =
        "--- a/hello.txt\n+++ b/hello.txt\n@@ -1 +1 @@\n-nonexistent line\n+hello aptu\n";
    let diff_path = w.join("bad.diff");
    std::fs::write(&diff_path, bad_diff).expect("write bad diff");

    let result = apply_patch_and_push(
        &diff_path,
        w,
        Some("test/bad-patch"),
        "main",
        "test: bad patch",
        false,
        false,
        false,
        |_| {},
    )
    .await;

    assert!(
        matches!(result, Err(PatchError::ApplyCheckFailed { .. })),
        "expected ApplyCheckFailed, got: {result:?}"
    );
}

/// Branch collision: if the branch already exists on origin, a date suffix is appended.
#[tokio::test]
async fn test_apply_patch_branch_collision_suffix() {
    let (work, _bare) = setup_repo();
    let w = work.path();

    // Pre-create the target branch on origin by pushing a dummy branch.
    git(w, &["checkout", "-b", "test/collision", "origin/main"]);
    git(w, &["push", "origin", "test/collision"]);
    // Return to main for the test.
    git(w, &["checkout", "main"]);

    let diff_path = write_diff(w);

    let branch = apply_patch_and_push(
        &diff_path,
        w,
        Some("test/collision"),
        "main",
        "test: collision",
        false,
        false,
        false,
        |_| {},
    )
    .await
    .expect("apply_patch_and_push should succeed with suffixed branch");

    // The returned branch must differ from the original and include a date-like suffix.
    assert_ne!(
        branch, "test/collision",
        "expected a suffixed branch name, got: {branch}"
    );
    assert!(
        branch.starts_with("test/collision-"),
        "expected suffix after 'test/collision', got: {branch}"
    );
}

/// DCO sign-off: `--signoff` appears in the commit log when `dco_signoff = true`.
#[tokio::test]
async fn test_apply_patch_dco_signoff() {
    let (work, _bare) = setup_repo();
    let w = work.path();
    let diff_path = write_diff(w);

    apply_patch_and_push(
        &diff_path,
        w,
        Some("test/dco"),
        "main",
        "test: dco signoff",
        true, // dco_signoff = true
        false,
        false,
        |_| {},
    )
    .await
    .expect("apply_patch_and_push should succeed");

    // Check commit message contains Signed-off-by trailer.
    let log = Command::new("git")
        .args(["log", "--format=%B", "-n", "1", "test/dco"])
        .current_dir(w)
        .output()
        .expect("git log");
    let log_str = String::from_utf8_lossy(&log.stdout);
    assert!(
        log_str.contains("Signed-off-by:"),
        "expected Signed-off-by trailer in commit, got: {log_str}"
    );
}

/// GPG signing gate: when `commit.gpgSign = true`, the commit step passes `-S`.
///
/// This test is `#[ignore]` because it requires a real GPG key in the test
/// environment. Enable it locally with `cargo test -- --ignored`.
#[tokio::test]
#[ignore]
async fn test_apply_patch_signing_gate() {
    let (work, _bare) = setup_repo();
    let w = work.path();

    // Enable GPG signing in the test repo.
    git(w, &["config", "commit.gpgSign", "true"]);

    let diff_path = write_diff(w);

    let branch = apply_patch_and_push(
        &diff_path,
        w,
        Some("test/gpg"),
        "main",
        "test: gpg signing",
        false,
        false,
        false,
        |_| {},
    )
    .await
    .expect("apply_patch_and_push should succeed with GPG signing");

    assert_eq!(branch, "test/gpg");
}
