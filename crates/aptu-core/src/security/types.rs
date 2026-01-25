// SPDX-License-Identifier: Apache-2.0

//! Security scan types and data structures.

use serde::{Deserialize, Serialize};

/// Severity level of a security finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Critical security vulnerability requiring immediate attention.
    Critical,
    /// High severity issue that should be addressed soon.
    High,
    /// Medium severity issue.
    Medium,
    /// Low severity issue or informational finding.
    #[default]
    Low,
}

/// Confidence level of a security finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// High confidence - very likely a real issue.
    High,
    /// Medium confidence - may require manual review.
    Medium,
    /// Low confidence - may be a false positive.
    #[default]
    Low,
}

/// A security finding from pattern matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Finding {
    /// Pattern ID that matched.
    #[serde(default)]
    pub pattern_id: String,
    /// Human-readable description of the issue.
    #[serde(default)]
    pub description: String,
    /// Severity level.
    #[serde(default)]
    pub severity: Severity,
    /// Confidence level.
    #[serde(default)]
    pub confidence: Confidence,
    /// File path where the finding was detected.
    #[serde(default)]
    pub file_path: String,
    /// Line number in the file (1-indexed).
    #[serde(default)]
    pub line_number: usize,
    /// The matched code snippet.
    #[serde(default)]
    pub matched_text: String,
    /// Optional CWE identifier (e.g., "CWE-798").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwe: Option<String>,
}

/// Pattern definition for security scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDefinition {
    /// Unique identifier for this pattern.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Regex pattern to match.
    pub pattern: String,
    /// Severity level for matches.
    pub severity: Severity,
    /// Confidence level for matches.
    pub confidence: Confidence,
    /// Optional CWE identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwe: Option<String>,
    /// File extensions to scan (empty = all files).
    #[serde(default)]
    pub file_extensions: Vec<String>,
}

/// A security finding that has been validated by LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidatedFinding {
    /// Original finding from pattern matching.
    #[serde(flatten)]
    pub finding: Finding,
    /// Whether the LLM confirmed this as a real issue.
    #[serde(default)]
    pub is_valid: bool,
    /// LLM's reasoning for the validation decision.
    #[serde(default)]
    pub reasoning: String,
    /// Model version used for validation (e.g., "anthropic/claude-3.5-sonnet").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

/// LLM validation result for a single finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Index of the finding in the batch (0-based).
    pub index: usize,
    /// Whether the finding is valid.
    pub is_valid: bool,
    /// Reasoning for the decision.
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finding_serialization() {
        let finding = Finding {
            pattern_id: "hardcoded-secret".to_string(),
            description: "Hardcoded API key detected".to_string(),
            severity: Severity::Critical,
            confidence: Confidence::High,
            file_path: "src/config.rs".to_string(),
            line_number: 42,
            matched_text: "api_key = \"sk-1234567890\"".to_string(),
            cwe: Some("CWE-798".to_string()),
        };

        let json = serde_json::to_string(&finding).unwrap();
        let deserialized: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(finding, deserialized);
    }

    #[test]
    fn test_severity_serialization() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"critical\""
        );
        assert_eq!(serde_json::to_string(&Severity::High).unwrap(), "\"high\"");
    }

    #[test]
    fn test_confidence_serialization() {
        assert_eq!(
            serde_json::to_string(&Confidence::High).unwrap(),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&Confidence::Medium).unwrap(),
            "\"medium\""
        );
    }

    #[test]
    fn test_pattern_definition_deserialization() {
        let json = r#"{
            "id": "test-pattern",
            "description": "Test pattern",
            "pattern": "test.*regex",
            "severity": "high",
            "confidence": "medium",
            "cwe": "CWE-123",
            "file_extensions": [".rs", ".py"]
        }"#;

        let pattern: PatternDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(pattern.id, "test-pattern");
        assert_eq!(pattern.severity, Severity::High);
        assert_eq!(pattern.confidence, Confidence::Medium);
        assert_eq!(pattern.cwe, Some("CWE-123".to_string()));
        assert_eq!(pattern.file_extensions, vec![".rs", ".py"]);
    }

    #[test]
    fn test_validated_finding_default() {
        let validated = ValidatedFinding::default();
        assert_eq!(validated.finding, Finding::default());
        assert!(!validated.is_valid);
        assert_eq!(validated.reasoning, "");
        assert_eq!(validated.model_version, None);
    }
}
