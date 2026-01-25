// SPDX-License-Identifier: Apache-2.0

//! Security scanner orchestration for PR diffs.

use crate::security::patterns::PatternEngine;
use crate::security::types::Finding;

/// Security scanner for analyzing code changes.
#[derive(Debug)]
pub struct SecurityScanner {
    engine: &'static PatternEngine,
}

impl SecurityScanner {
    /// Creates a new security scanner using the global pattern engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: PatternEngine::global(),
        }
    }

    /// Scans a PR diff for security vulnerabilities.
    ///
    /// # Arguments
    ///
    /// * `diff` - The unified diff text from a pull request
    ///
    /// # Returns
    ///
    /// A vector of security findings from added/modified lines.
    #[must_use]
    pub fn scan_diff(&self, diff: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut current_file = String::new();
        let mut current_line_num = 0;

        for line in diff.lines() {
            // Track current file being processed
            if line.starts_with("+++") {
                // Extract file path from "+++ b/path/to/file"
                if let Some(path) = line.strip_prefix("+++ b/") {
                    current_file = path.to_string();
                }
                continue;
            }

            // Track line numbers from diff hunks
            if line.starts_with("@@") {
                // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
                if let Some(new_pos) = line.split('+').nth(1)
                    && let Some(line_num_str) = new_pos.split(',').next()
                {
                    current_line_num = line_num_str
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                }
                continue;
            }

            // Only scan added lines (starting with '+')
            if let Some(code) = line.strip_prefix('+') {
                // Skip if it's the file marker line
                if code.starts_with("++") {
                    continue;
                }

                // Scan the added line
                let line_findings = self.engine.scan(code, &current_file);
                for mut finding in line_findings {
                    // Override line number with actual diff position
                    finding.line_number = current_line_num;
                    findings.push(finding);
                }

                current_line_num += 1;
            } else if !line.starts_with('-') && !line.starts_with('\\') {
                // Context lines (no prefix) also increment line number
                current_line_num += 1;
            }
        }

        findings
    }

    /// Scans file content directly (not a diff).
    ///
    /// # Arguments
    ///
    /// * `content` - The file content to scan
    /// * `file_path` - Path to the file
    ///
    /// # Returns
    ///
    /// A vector of security findings.
    #[must_use]
    pub fn scan_file(&self, content: &str, file_path: &str) -> Vec<Finding> {
        self.engine.scan(content, file_path)
    }
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_creation() {
        let scanner = SecurityScanner::new();
        assert!(scanner.engine.pattern_count() > 0);
    }

    #[test]
    fn test_scan_file() {
        let scanner = SecurityScanner::new();
        let code = r#"
            let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
        "#;

        let findings = scanner.scan_file(code, "config.rs");
        assert!(!findings.is_empty(), "Should detect hardcoded secret");
    }

    #[test]
    fn test_scan_diff_basic() {
        let scanner = SecurityScanner::new();
        let diff = r#"
diff --git a/src/config.rs b/src/config.rs
index 1234567..abcdefg 100644
--- a/src/config.rs
+++ b/src/config.rs
@@ -10,3 +10,4 @@ fn load_config() {
     let host = "localhost";
+    let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
 }
"#;

        let findings = scanner.scan_diff(diff);
        assert!(
            !findings.is_empty(),
            "Should detect hardcoded API key in diff"
        );
        assert_eq!(findings[0].file_path, "src/config.rs");
    }

    #[test]
    fn test_scan_diff_ignores_removed_lines() {
        let scanner = SecurityScanner::new();
        let diff = r#"
diff --git a/src/old.rs b/src/old.rs
--- a/src/old.rs
+++ b/src/old.rs
@@ -1,2 +1,1 @@
-let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
+let api_key = env::var("API_KEY").unwrap();
"#;

        let findings = scanner.scan_diff(diff);
        // Should not detect the removed line (with '-' prefix)
        // Should only scan the added line which is safe
        assert!(
            findings.is_empty(),
            "Should not detect secrets in removed lines"
        );
    }

    #[test]
    fn test_scan_diff_multiple_files() {
        let scanner = SecurityScanner::new();
        let diff = r#"
diff --git a/src/auth.rs b/src/auth.rs
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -1,1 +1,2 @@
 fn authenticate() {
+    let password = "hardcoded123";
 }
diff --git a/src/db.rs b/src/db.rs
--- a/src/db.rs
+++ b/src/db.rs
@@ -1,1 +1,2 @@
 fn query_user(id: &str) {
+    execute("SELECT * FROM users WHERE id = " + id);
 }
"#;

        let findings = scanner.scan_diff(diff);
        assert!(
            findings.len() >= 2,
            "Should detect issues in multiple files"
        );

        let auth_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.file_path == "src/auth.rs")
            .collect();
        assert!(!auth_findings.is_empty(), "Should find issue in auth.rs");

        let db_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.file_path == "src/db.rs")
            .collect();
        assert!(!db_findings.is_empty(), "Should find issue in db.rs");
    }

    #[test]
    fn test_scan_diff_line_numbers() {
        let scanner = SecurityScanner::new();
        let diff = r#"
diff --git a/test.rs b/test.rs
--- a/test.rs
+++ b/test.rs
@@ -5,2 +5,3 @@ fn main() {
     println!("line 5");
     println!("line 6");
+    let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
"#;

        let findings = scanner.scan_diff(diff);
        assert_eq!(findings.len(), 1);
        // The added line should be at line 7 (after lines 5 and 6)
        assert_eq!(findings[0].line_number, 7);
    }

    #[test]
    fn test_scan_empty_diff() {
        let scanner = SecurityScanner::new();
        let findings = scanner.scan_diff("");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_default_constructor() {
        let scanner = SecurityScanner::default();
        assert!(scanner.engine.pattern_count() > 0);
    }
}
