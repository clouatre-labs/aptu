// SPDX-License-Identifier: Apache-2.0

//! Security pattern engine with regex-based vulnerability detection.

use crate::security::types::{Finding, PatternDefinition};
use regex::Regex;
use std::sync::LazyLock;

/// Embedded pattern database JSON.
const PATTERNS_JSON: &str = include_str!("patterns.json");

/// Compiled pattern engine (initialized once on first use).
static PATTERN_ENGINE: LazyLock<PatternEngine> = LazyLock::new(|| {
    PatternEngine::from_embedded_json()
        .expect("Failed to load embedded security patterns - patterns.json is malformed")
});

/// Pattern engine for security scanning.
#[derive(Debug)]
pub struct PatternEngine {
    patterns: Vec<CompiledPattern>,
}

/// A pattern with pre-compiled regex.
#[derive(Debug)]
struct CompiledPattern {
    definition: PatternDefinition,
    regex: Regex,
}

impl PatternEngine {
    /// Creates a pattern engine from the embedded JSON patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is malformed or regex compilation fails.
    pub fn from_embedded_json() -> anyhow::Result<Self> {
        let definitions: Vec<PatternDefinition> = serde_json::from_str(PATTERNS_JSON)?;
        let mut patterns = Vec::new();

        for def in definitions {
            let regex = Regex::new(&def.pattern)?;
            patterns.push(CompiledPattern {
                definition: def,
                regex,
            });
        }

        Ok(Self { patterns })
    }

    /// Gets the global pattern engine instance.
    #[must_use]
    pub fn global() -> &'static Self {
        &PATTERN_ENGINE
    }

    /// Scans text content for security vulnerabilities.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content to scan
    /// * `file_path` - Path to the file being scanned (for filtering and reporting)
    ///
    /// # Returns
    ///
    /// A vector of security findings.
    pub fn scan(&self, content: &str, file_path: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let file_ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"));

        for (line_num, line) in content.lines().enumerate() {
            for compiled in &self.patterns {
                // Skip if pattern has file extension filter and doesn't match
                if !compiled.definition.file_extensions.is_empty() {
                    if let Some(ref ext) = file_ext {
                        if !compiled.definition.file_extensions.contains(ext) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                if let Some(mat) = compiled.regex.find(line) {
                    tracing::debug!(
                        pattern_id = %compiled.definition.id,
                        file = %file_path,
                        line = line_num + 1,
                        "Security pattern matched"
                    );

                    findings.push(Finding {
                        pattern_id: compiled.definition.id.clone(),
                        description: compiled.definition.description.clone(),
                        severity: compiled.definition.severity,
                        confidence: compiled.definition.confidence,
                        file_path: file_path.to_string(),
                        line_number: line_num + 1,
                        matched_text: mat.as_str().to_string(),
                        cwe: compiled.definition.cwe.clone(),
                    });
                }
            }
        }

        findings
    }

    /// Returns the number of loaded patterns.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::types::{Confidence, Severity};

    #[test]
    fn test_pattern_engine_loads() {
        let engine = PatternEngine::from_embedded_json().unwrap();
        assert!(
            engine.pattern_count() >= 10,
            "Should have at least 10 patterns"
        );
    }

    #[test]
    fn test_global_engine() {
        let engine = PatternEngine::global();
        assert!(engine.pattern_count() >= 10);
    }

    #[test]
    fn test_hardcoded_api_key_detection() {
        let engine = PatternEngine::global();
        let code = r#"
            let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
            let secret_key = "secret_1234567890abcdefghij";
        "#;

        let findings = engine.scan(code, "test.rs");
        assert!(!findings.is_empty(), "Should detect hardcoded secrets");

        let api_key_finding = findings
            .iter()
            .find(|f| f.pattern_id == "hardcoded-api-key");
        assert!(api_key_finding.is_some(), "Should detect API key");

        if let Some(finding) = api_key_finding {
            assert_eq!(finding.severity, Severity::Critical);
            assert_eq!(finding.confidence, Confidence::High);
            assert_eq!(finding.cwe, Some("CWE-798".to_string()));
        }
    }

    #[test]
    fn test_sql_injection_detection() {
        let engine = PatternEngine::global();
        let code = r#"
            query("SELECT * FROM users WHERE id = " + user_input);
            execute(format!("DELETE FROM {} WHERE id = {}", table, id));
        "#;

        let findings = engine.scan(code, "database.rs");
        assert!(!findings.is_empty(), "Should detect SQL injection patterns");

        let concat_finding = findings
            .iter()
            .find(|f| f.pattern_id == "sql-injection-concat");
        assert!(concat_finding.is_some(), "Should detect concatenation");

        let format_finding = findings
            .iter()
            .find(|f| f.pattern_id == "sql-injection-format");
        assert!(format_finding.is_some(), "Should detect format string");
    }

    #[test]
    fn test_path_traversal_detection() {
        let engine = PatternEngine::global();
        let code = r#"
            open("../../etc/passwd");
            read("..\..\..\windows\system32\config\sam");
        "#;

        let findings = engine.scan(code, "file_handler.rs");
        assert!(!findings.is_empty(), "Should detect path traversal");

        let finding = &findings[0];
        assert_eq!(finding.pattern_id, "path-traversal");
        assert_eq!(finding.severity, Severity::High);
    }

    #[test]
    fn test_weak_crypto_detection() {
        let engine = PatternEngine::global();
        let code = r"
            let hash = md5(password);
            let digest = SHA1(data);
        ";

        let findings = engine.scan(code, "crypto.rs");
        assert_eq!(findings.len(), 2, "Should detect both MD5 and SHA1");

        assert!(findings.iter().any(|f| f.pattern_id == "weak-crypto-md5"));
        assert!(findings.iter().any(|f| f.pattern_id == "weak-crypto-sha1"));
    }

    #[test]
    fn test_file_extension_filtering() {
        let engine = PatternEngine::global();
        let js_code = "element.innerHTML = userInput + '<div>';";

        // Should detect in .js file
        let js_findings = engine.scan(js_code, "app.js");
        assert!(!js_findings.is_empty(), "Should detect XSS in JS file");

        // Should NOT detect in .rs file (pattern has file extension filter)
        let rs_findings = engine.scan(js_code, "app.rs");
        assert!(
            rs_findings.is_empty(),
            "Should not detect XSS pattern in Rust file"
        );
    }

    #[test]
    fn test_no_false_positives_on_safe_code() {
        let engine = PatternEngine::global();
        let safe_code = r#"
            // Safe code examples
            let config = load_config();
            let result = query_with_params("SELECT * FROM users WHERE id = ?", &[id]);
            let hash = sha256(data);
            let random = OsRng.gen::<u64>();
        "#;

        let findings = engine.scan(safe_code, "safe.rs");
        assert!(
            findings.is_empty(),
            "Should not have false positives on safe code"
        );
    }

    #[test]
    fn test_line_number_accuracy() {
        let engine = PatternEngine::global();
        let code = "line 1\nline 2\napi_key = \"sk-1234567890abcdefghijklmnopqrstuvwxyz\"\nline 4";

        let findings = engine.scan(code, "test.rs");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].line_number, 3,
            "Should report correct line number"
        );
    }
}
