// SPDX-License-Identifier: Apache-2.0

//! Smart detection logic for when to trigger security scans.

/// Determines if a security scan should be performed based on context.
///
/// Checks file paths, PR labels, and description keywords to decide if
/// security scanning is warranted.
///
/// # Arguments
///
/// * `file_paths` - List of file paths changed in the PR
/// * `labels` - PR labels
/// * `description` - PR title and body text
///
/// # Returns
///
/// `true` if a security scan should be performed.
#[must_use]
pub fn needs_security_scan(file_paths: &[String], labels: &[String], description: &str) -> bool {
    // Check for security-related labels
    if labels.iter().any(|label| {
        let lower = label.to_lowercase();
        lower.contains("security")
            || lower.contains("vulnerability")
            || lower.contains("cve")
            || lower.contains("exploit")
    }) {
        return true;
    }

    // Check for security keywords in description
    let desc_lower = description.to_lowercase();
    if desc_lower.contains("security")
        || desc_lower.contains("vulnerability")
        || desc_lower.contains("exploit")
        || desc_lower.contains("injection")
        || desc_lower.contains("xss")
        || desc_lower.contains("csrf")
        || desc_lower.contains("authentication")
        || desc_lower.contains("authorization")
        || desc_lower.contains("crypto")
        || desc_lower.contains("password")
        || desc_lower.contains("secret")
        || desc_lower.contains("token")
    {
        return true;
    }

    // Check for sensitive file paths
    for path in file_paths {
        let path_lower = path.to_lowercase();

        // Security-related directories
        if path_lower.contains("/auth")
            || path_lower.contains("/security")
            || path_lower.contains("/crypto")
            || path_lower.contains("/password")
        {
            return true;
        }

        // Configuration files that might contain secrets
        let path_obj = std::path::Path::new(&path_lower);
        if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("env"))
            || path_lower.ends_with(".env.example")
            || path_lower.contains("config")
            || path_lower.contains("secret")
            || path_lower.contains("credential")
        {
            return true;
        }

        // Database or SQL files
        if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
            || path_lower.contains("migration")
            || path_lower.contains("database")
        {
            return true;
        }

        // Authentication/authorization code
        if path_lower.contains("login")
            || path_lower.contains("signin")
            || path_lower.contains("signup")
            || path_lower.contains("register")
        {
            return true;
        }
    }

    // Default: no scan needed
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_label_triggers_scan() {
        assert!(needs_security_scan(&[], &["security".to_string()], ""));
        assert!(needs_security_scan(&[], &["vulnerability".to_string()], ""));
        assert!(needs_security_scan(
            &[],
            &["bug".to_string(), "Security Fix".to_string()],
            ""
        ));
    }

    #[test]
    fn test_description_keywords_trigger_scan() {
        assert!(needs_security_scan(
            &[],
            &[],
            "Fix security vulnerability in auth"
        ));
        assert!(needs_security_scan(
            &[],
            &[],
            "Prevent SQL injection attack"
        ));
        assert!(needs_security_scan(
            &[],
            &[],
            "Update password hashing algorithm"
        ));
        assert!(needs_security_scan(&[], &[], "Remove hardcoded API token"));
    }

    #[test]
    fn test_sensitive_file_paths_trigger_scan() {
        assert!(needs_security_scan(
            &["src/auth/login.rs".to_string()],
            &[],
            ""
        ));
        assert!(needs_security_scan(
            &["config/secrets.yml".to_string()],
            &[],
            ""
        ));
        assert!(needs_security_scan(&[".env.example".to_string()], &[], ""));
        assert!(needs_security_scan(
            &["migrations/001_users.sql".to_string()],
            &[],
            ""
        ));
        assert!(needs_security_scan(
            &["src/security/scanner.rs".to_string()],
            &[],
            ""
        ));
    }

    #[test]
    fn test_no_scan_for_regular_changes() {
        assert!(!needs_security_scan(
            &["README.md".to_string()],
            &[],
            "Update documentation"
        ));
        assert!(!needs_security_scan(
            &["src/utils.rs".to_string()],
            &["enhancement".to_string()],
            "Add helper function"
        ));
        assert!(!needs_security_scan(
            &["tests/test_utils.rs".to_string()],
            &["test".to_string()],
            "Add unit tests"
        ));
    }

    #[test]
    fn test_case_insensitive_matching() {
        assert!(needs_security_scan(&[], &["SECURITY".to_string()], ""));
        assert!(needs_security_scan(&[], &[], "SECURITY FIX"));
        assert!(needs_security_scan(
            &["SRC/AUTH/LOGIN.RS".to_string()],
            &[],
            ""
        ));
    }

    #[test]
    fn test_multiple_conditions() {
        // Multiple triggers should still return true
        assert!(needs_security_scan(
            &["src/auth/login.rs".to_string()],
            &["security".to_string()],
            "Fix authentication bug"
        ));
    }

    #[test]
    fn test_crypto_related_changes() {
        assert!(needs_security_scan(
            &["src/crypto/hash.rs".to_string()],
            &[],
            ""
        ));
        assert!(needs_security_scan(
            &[],
            &[],
            "Update cryptographic library"
        ));
    }
}
