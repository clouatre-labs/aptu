// SPDX-License-Identifier: Apache-2.0

//! Types for deterministic issue-body linting.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A lint specification for one issue template type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueLintSpec {
    /// Issue template type this spec applies to (TOML key `type`).
    #[serde(rename = "type")]
    pub issue_type: String,
    /// H2 headings that must appear in the issue body.
    #[serde(default)]
    pub required_headings: Vec<String>,
    /// Require at least one fenced code block or file-path reference.
    #[serde(default)]
    pub require_code_examples: bool,
    /// Require at least one external URL or `#N` issue reference.
    #[serde(default)]
    pub require_external_link: bool,
}

/// A single lint violation with an optional line hint for annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueLintViolation {
    /// Rule identifier (for example `spec/missing-heading`).
    pub rule: String,
    /// Human-readable description of the violation.
    pub message: String,
    /// 1-based line hint in the body, when meaningful.
    pub line: Option<usize>,
}

/// Result of linting an issue body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, JsonSchema)]
pub struct IssueLintResult {
    /// True when no violations were found.
    pub passed: bool,
    /// All violations, in check order.
    pub violations: Vec<IssueLintViolation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// serde round-trips for spec/violation/result types.
    #[test]
    fn test_issue_lint_type_serialization() {
        // Arrange
        let spec = IssueLintSpec {
            issue_type: "feature".to_string(),
            required_headings: vec!["Summary".to_string()],
            require_code_examples: true,
            require_external_link: false,
        };

        // Act
        let json = serde_json::to_string(&spec).expect("serialize spec");
        let parsed: IssueLintSpec = serde_json::from_str(&json).expect("parse spec");
        let violation = IssueLintViolation {
            rule: "spec/missing-heading".to_string(),
            message: "missing heading: Summary".to_string(),
            line: None,
        };
        let violation_json = serde_json::to_string(&violation).expect("serialize violation");
        let parsed_violation: IssueLintViolation =
            serde_json::from_str(&violation_json).expect("parse violation");

        // Assert
        assert_eq!(parsed, spec);
        assert_eq!(parsed_violation, violation);
        assert!(json.contains("\"type\":\"feature\""));
    }
}
