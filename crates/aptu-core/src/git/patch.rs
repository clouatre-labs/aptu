// SPDX-License-Identifier: Apache-2.0

//! Patch application and Git utilities for automated patch deployment.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::security::scanner::SecurityScanner;

/// Steps reported via progress callback during `apply_patch_and_push`.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchStep {
    /// Checking git version for CVE-2023-23946 compatibility.
    CheckingGitVersion,
    /// Validating patch file integrity and path safety.
    ValidatingPatch,
    /// Scanning patch content for security findings.
    SecurityScan,
    /// Performing dry-run application check.
    DryRunCheck,
    /// Creating feature branch from base.
    CreatingBranch,
    /// Applying patch to working directory.
    ApplyingPatch,
    /// Creating signed commit with patch changes.
    Committing,
    /// Pushing branch to origin remote.
    Pushing,
}

/// Errors returned by patch operations.
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// Patch file not found.
    #[error("patch file not found: {0}")]
    NotFound(PathBuf),
    /// Patch file exceeds 50MB limit.
    #[error("patch file too large ({size} bytes); maximum is 50MB")]
    TooLarge {
        /// Actual file size in bytes.
        size: u64,
    },
    /// Patch contains unsafe path traversal.
    #[error("patch contains unsafe path: {path} - refusing to apply")]
    PathTraversal {
        /// The offending path extracted from the patch header.
        path: String,
    },
    /// Patch attempts to create a symlink.
    #[error("patch creates a symlink ({path}) - refusing to apply")]
    SymlinkMode {
        /// The offending path extracted from the patch header.
        path: String,
    },
    /// Security scanner found issues in patch.
    #[error("security findings in patch ({count}). Pass --force to apply anyway.")]
    SecurityFindings {
        /// Number of security findings detected.
        count: usize,
    },
    /// Patch does not apply cleanly to target branch.
    #[error("patch does not apply cleanly:\n{detail}")]
    ApplyCheckFailed {
        /// Stderr output from `git apply --check`.
        detail: String,
    },
    /// Branch name already exists on origin.
    #[error("branch {name} already exists. Use --branch to specify a different name.")]
    BranchCollision {
        /// The branch name that collided.
        name: String,
    },
    /// Git version is too old for safe patching.
    #[error("git >= 2.39.2 required (found {version}). CVE-2023-23946 is unpatched.")]
    GitTooOld {
        /// The version string reported by the system git binary.
        version: String,
    },
    /// Git command execution failed.
    #[error("git command failed: {detail}")]
    GitFailed {
        /// Stderr output from the failed git invocation.
        detail: String,
    },
    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Run a git command in the given directory. Returns trimmed stdout on success.
fn run_git(args: &[&str], cwd: &Path) -> Result<String, PatchError> {
    let output = Command::new("git").args(args).current_dir(cwd).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(PatchError::GitFailed {
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Read a git config value. Returns None if not set.
fn git_config_get(key: &str, cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", key])
        .current_dir(cwd)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Parse a git version string like `"git version 2.39.2"` or `"git version 2.46.2 (Apple Git-139)"`.
/// Returns `Err(PatchError::GitTooOld)` if version < 2.39.2 or if parsing fails.
pub fn parse_git_version_str(s: &str) -> Result<(), PatchError> {
    // "git version 2.39.2" or "git version 2.46.2 (Apple Git-139)"
    let version_part = s
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| PatchError::GitTooOld {
            version: s.to_string(),
        })?
        .split('(')
        .next()
        .ok_or_else(|| PatchError::GitTooOld {
            version: s.to_string(),
        })?
        .trim_end_matches('.')
        .to_string();

    let parts: Vec<u64> = version_part
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();

    let (major, minor, patch) = match parts.as_slice() {
        [ma, mi, pa, ..] => (*ma, *mi, *pa),
        [ma, mi] => (*ma, *mi, 0),
        [ma] => (*ma, 0, 0),
        [] => {
            return Err(PatchError::GitTooOld {
                version: version_part,
            });
        }
    };

    // Required: >= 2.39.2
    let ok = (major, minor, patch) >= (2, 39, 2);
    if ok {
        Ok(())
    } else {
        Err(PatchError::GitTooOld {
            version: version_part,
        })
    }
}

/// Check that the system git binary is >= 2.39.2 (CVE-2023-23946 patched).
pub fn git_version_check(cwd: &Path) -> Result<(), PatchError> {
    let output = Command::new("git")
        .arg("--version")
        .current_dir(cwd)
        .output()?;
    let s = String::from_utf8_lossy(&output.stdout).to_string();
    parse_git_version_str(&s)
}

/// Validate patch file path headers for traversal and symlink attacks.
pub fn validate_patch_paths(content: &str) -> Result<(), PatchError> {
    for line in content.lines() {
        // Check for symlink mode creation
        if line.starts_with("new file mode 120000") {
            // Find path from previous +++ or --- line -- but just flag on this line
            return Err(PatchError::SymlinkMode {
                path: "(symlink)".to_string(),
            });
        }
        // Check +++ b/<path> headers
        if let Some(path) = line.strip_prefix("+++ b/") {
            let path = path.trim();
            if path.starts_with('/') || path.contains("../") || path.contains("\\..") {
                return Err(PatchError::PathTraversal {
                    path: path.to_string(),
                });
            }
        }
        // Also check --- a/<path> headers
        if let Some(path) = line.strip_prefix("--- a/") {
            let path = path.trim();
            if path.starts_with('/') || path.contains("../") || path.contains("\\..") {
                return Err(PatchError::PathTraversal {
                    path: path.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Slugify a PR title into a valid git branch name segment.
/// Follows the algorithm from issue #1126.
#[must_use]
pub fn slugify_title(title: &str) -> String {
    // Lowercase, drop non-ASCII
    let lower: String = title
        .chars()
        .filter(char::is_ascii)
        .collect::<String>()
        .to_lowercase();

    // Replace runs of non-alnum with hyphens
    let mut slug = String::new();
    let mut last_hyphen = true; // suppress leading hyphens
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            last_hyphen = false;
            slug.push(c);
        } else if !last_hyphen {
            slug.push('-');
            last_hyphen = true;
        }
    }
    // Strip trailing hyphen
    let slug = slug.trim_end_matches('-').to_string();
    let slug = slug.trim_start_matches('-').to_string();

    // Detect conventional-commit prefix
    let conventional_prefixes = [
        "feat", "fix", "docs", "chore", "test", "refactor", "perf", "ci", "build", "style",
    ];
    let mut result = slug.clone();
    for prefix in &conventional_prefixes {
        let slug_prefix_with_hyphen = format!("{prefix}-");
        if slug.starts_with(&slug_prefix_with_hyphen) {
            // Replace "feat-" prefix with "feat/"
            result = format!("{}/{}", prefix, &slug[slug_prefix_with_hyphen.len()..]);
            break;
        }
    }

    // Truncate to 60 chars at a hyphen/slash boundary
    if result.len() > 60 {
        let truncated = &result[..60];
        // Find last hyphen or slash within the 60-char window
        if let Some(pos) = truncated.rfind(&['-', '/'][..]) {
            result = truncated[..pos].to_string();
        } else {
            result = truncated.to_string();
        }
    }

    result
}

/// Apply a patch file, commit, and push to origin. Returns the branch name that was pushed.
#[allow(clippy::too_many_arguments)]
pub async fn apply_patch_and_push(
    patch_path: &Path,
    repo_root: &Path,
    branch: Option<&str>,
    base: &str,
    title: &str,
    dco_signoff: bool,
    force: bool,
    dry_run: bool,
    progress: impl Fn(PatchStep),
) -> Result<String, PatchError> {
    const MAX_SIZE: u64 = 50 * 1024 * 1024; // 50MB

    // Step 1: git version check
    progress(PatchStep::CheckingGitVersion);
    git_version_check(repo_root)?;

    // Step 2: Validate patch
    progress(PatchStep::ValidatingPatch);
    if !patch_path.exists() {
        return Err(PatchError::NotFound(patch_path.to_path_buf()));
    }
    let metadata = std::fs::metadata(patch_path)?;
    let size = metadata.len();
    if size > MAX_SIZE {
        return Err(PatchError::TooLarge { size });
    }
    let content = std::fs::read_to_string(patch_path)?;
    validate_patch_paths(&content)?;

    // Step 3: Security scan
    progress(PatchStep::SecurityScan);
    let scanner = SecurityScanner::new();
    let findings = scanner.scan_diff(&content);
    if !findings.is_empty() && !force {
        return Err(PatchError::SecurityFindings {
            count: findings.len(),
        });
    }

    // Step 4: Dry-run apply check
    progress(PatchStep::DryRunCheck);
    let patch_abs = patch_path
        .canonicalize()
        .unwrap_or_else(|_| patch_path.to_path_buf());
    let patch_str = patch_abs.to_string_lossy();
    let check_output = Command::new("git")
        .args(["apply", "--check", &patch_str])
        .current_dir(repo_root)
        .output()?;
    if !check_output.status.success() {
        return Err(PatchError::ApplyCheckFailed {
            detail: String::from_utf8_lossy(&check_output.stderr)
                .trim()
                .to_string(),
        });
    }

    // Derive branch name
    let branch_name = branch.map_or_else(|| slugify_title(title), str::to_owned);

    // Collision check and suffix logic
    let branch_name = resolve_branch_name(&branch_name, repo_root)?;

    // Early return for dry-run (no side effects past this point)
    if dry_run {
        return Ok(branch_name);
    }

    // Step 5: Create branch
    progress(PatchStep::CreatingBranch);
    let base_ref = format!("origin/{base}");
    run_git(&["checkout", "-b", &branch_name, &base_ref], repo_root)?;

    // Step 6: Apply patch
    progress(PatchStep::ApplyingPatch);
    run_git(&["apply", &patch_str], repo_root)?;

    // Stage all changes
    run_git(&["add", "-A"], repo_root)?;

    // Step 7: Commit
    progress(PatchStep::Committing);
    let gpg_sign =
        git_config_get("commit.gpgSign", repo_root).is_some_and(|v| v.eq_ignore_ascii_case("true"));

    let mut commit_args: Vec<String> =
        vec!["commit".to_string(), "-m".to_string(), title.to_string()];
    if gpg_sign {
        commit_args.push("-S".to_string());
    }
    if dco_signoff {
        commit_args.push("--signoff".to_string());
    }
    let commit_args_ref: Vec<&str> = commit_args.iter().map(String::as_str).collect();
    run_git(&commit_args_ref, repo_root)?;

    // Step 8: Push
    progress(PatchStep::Pushing);
    run_git(&["push", "origin", &branch_name], repo_root)?;

    Ok(branch_name)
}

/// Resolve branch name with collision handling.
fn resolve_branch_name(name: &str, repo_root: &Path) -> Result<String, PatchError> {
    use std::time::{SystemTime, UNIX_EPOCH};

    if !branch_exists_remote(name, repo_root) {
        return Ok(name.to_string());
    }

    // Try with date suffix
    let date = chrono::Utc::now().format("%Y%m%d").to_string();
    let with_date = format!("{name}-{date}");
    if !branch_exists_remote(&with_date, repo_root) {
        return Ok(with_date);
    }

    // Try with random hex suffix
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let hex_suffix = format!("{seed:06x}");
    let with_hex = format!("{name}-{hex_suffix}");
    if !branch_exists_remote(&with_hex, repo_root) {
        return Ok(with_hex);
    }

    Err(PatchError::BranchCollision {
        name: name.to_string(),
    })
}

fn branch_exists_remote(name: &str, repo_root: &Path) -> bool {
    let refspec = format!("refs/heads/{name}");
    let output = Command::new("git")
        .args(["ls-remote", "origin", &refspec])
        .current_dir(repo_root)
        .output()
        .ok();
    output.is_some_and(|o| o.status.success() && !o.stdout.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_version_parse_valid() {
        assert!(parse_git_version_str("git version 2.39.2\n").is_ok());
        assert!(parse_git_version_str("git version 2.43.0\n").is_ok());
    }

    #[test]
    fn test_git_version_parse_apple_git() {
        assert!(parse_git_version_str("git version 2.46.2 (Apple Git-139)\n").is_ok());
    }

    #[test]
    fn test_git_version_too_old() {
        let err = parse_git_version_str("git version 2.38.0\n");
        assert!(matches!(err, Err(PatchError::GitTooOld { .. })));
    }

    #[test]
    fn test_validate_patch_paths_safe() {
        let diff = "+++ b/src/main.rs\n--- a/src/main.rs\n";
        assert!(validate_patch_paths(diff).is_ok());
    }

    #[test]
    fn test_validate_patch_paths_traversal() {
        let diff = "+++ b/../etc/passwd\n";
        let err = validate_patch_paths(diff);
        assert!(matches!(err, Err(PatchError::PathTraversal { .. })));
    }

    #[test]
    fn test_validate_patch_paths_absolute() {
        let diff = "+++ b//etc/shadow\n";
        let err = validate_patch_paths(diff);
        assert!(matches!(err, Err(PatchError::PathTraversal { .. })));
    }

    #[test]
    fn test_validate_patch_paths_symlink_mode() {
        let diff = "new file mode 120000\n";
        let err = validate_patch_paths(diff);
        assert!(matches!(err, Err(PatchError::SymlinkMode { .. })));
    }

    #[test]
    fn test_slugify_basic() {
        // "Fix login bug" -> lowercase -> "fix-login-bug" -> detect "fix-" prefix -> "fix/login-bug"
        assert_eq!(slugify_title("Fix login bug"), "fix/login-bug");
    }

    #[test]
    fn test_slugify_conventional_prefix() {
        assert_eq!(slugify_title("fix: add retry logic"), "fix/add-retry-logic");
    }

    #[test]
    fn test_slugify_truncation() {
        let long_title = "feat: this is a very long title that exceeds sixty characters limit here";
        let result = slugify_title(long_title);
        assert!(result.len() <= 60, "slug too long: {result}");
    }
}
