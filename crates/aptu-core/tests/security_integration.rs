// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `SecurityScanner` using fixture files.
//!
//! These tests verify that the security scanner correctly detects vulnerabilities
//! in vulnerable fixtures and produces zero findings for safe fixtures.

use aptu_core::security::scanner::SecurityScanner;
use std::fmt::Write;

/// Test fixture: `hardcoded_secrets.rs`
const HARDCODED_SECRETS_FIXTURE: &str =
    include_str!("../../../tests/security_fixtures/vulnerable/hardcoded_secrets.rs");

/// Test fixture: `sql_injection.rs`
const SQL_INJECTION_FIXTURE: &str =
    include_str!("../../../tests/security_fixtures/vulnerable/sql_injection.rs");

/// Test fixture: `safe_patterns.rs`
const SAFE_PATTERNS_FIXTURE: &str =
    include_str!("../../../tests/security_fixtures/safe/safe_patterns.rs");

/// Helper function to generate a unified diff format for testing.
///
/// Takes fixture content and a filename, returns a properly formatted diff
/// that can be passed to `SecurityScanner::scan_diff()`.
fn create_test_diff(content: &str, filename: &str) -> String {
    let mut diff_content = String::new();
    for line in content.lines() {
        let _ = writeln!(diff_content, "+{line}");
    }
    format!(
        r#"diff --git a/{filename} b/{filename}
index 0000000..1111111 100644
--- a/{filename}
+++ b/{filename}
@@ -0,0 +1,{line_count} @@
{diff_content}"#,
        line_count = content.lines().count(),
    )
}

#[test]
fn test_hardcoded_secrets_detection() {
    let scanner = SecurityScanner::new();
    let diff = create_test_diff(HARDCODED_SECRETS_FIXTURE, "test.rs");
    let findings = scanner.scan_diff(&diff);

    // Verify we detected hardcoded-api-key pattern
    let api_key_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "hardcoded-api-key")
        .collect();
    assert!(
        !api_key_findings.is_empty(),
        "Should detect hardcoded-api-key pattern in fixture. Findings: {findings:#?}"
    );

    // Verify we detected hardcoded-password pattern
    let password_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "hardcoded-password")
        .collect();
    assert!(
        !password_findings.is_empty(),
        "Should detect hardcoded-password pattern in fixture. Findings: {findings:#?}"
    );
}

#[test]
fn test_sql_injection_detection() {
    let scanner = SecurityScanner::new();
    let diff = create_test_diff(SQL_INJECTION_FIXTURE, "test.rs");
    let findings = scanner.scan_diff(&diff);

    // Verify we detected sql-injection-concat pattern
    let concat_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "sql-injection-concat")
        .collect();
    assert!(
        !concat_findings.is_empty(),
        "Should detect sql-injection-concat pattern in fixture. Findings: {findings:#?}"
    );

    // Verify we detected sql-injection-format pattern
    let format_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "sql-injection-format")
        .collect();
    assert!(
        !format_findings.is_empty(),
        "Should detect sql-injection-format pattern in fixture. Findings: {findings:#?}"
    );

    // Verify we detected command-injection pattern
    let cmd_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "command-injection")
        .collect();
    assert!(
        !cmd_findings.is_empty(),
        "Should detect command-injection pattern in fixture. Findings: {findings:#?}"
    );

    // Verify we detected weak-crypto-md5 pattern
    let md5_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "weak-crypto-md5")
        .collect();
    assert!(
        !md5_findings.is_empty(),
        "Should detect weak-crypto-md5 pattern in fixture. Findings: {findings:#?}"
    );

    // Verify we detected weak-crypto-sha1 pattern
    let sha1_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "weak-crypto-sha1")
        .collect();
    assert!(
        !sha1_findings.is_empty(),
        "Should detect weak-crypto-sha1 pattern in fixture. Findings: {findings:#?}"
    );
}

#[test]
fn test_safe_patterns_no_findings() {
    let scanner = SecurityScanner::new();
    let diff = create_test_diff(SAFE_PATTERNS_FIXTURE, "test.rs");
    let findings = scanner.scan_diff(&diff);

    assert!(
        findings.is_empty(),
        "Safe fixture should produce zero findings, but got: {findings:#?}"
    );
}
