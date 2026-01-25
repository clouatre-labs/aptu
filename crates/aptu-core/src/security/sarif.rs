// SPDX-License-Identifier: Apache-2.0

//! SARIF (Static Analysis Results Interchange Format) output support.
//!
//! Converts security findings to SARIF 2.1.0 format for integration with
//! GitHub Code Scanning and other security tools.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::Finding;

/// SARIF report structure (SARIF 2.1.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifReport {
    /// SARIF schema version.
    pub version: String,
    /// SARIF schema URI.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// List of runs (one per tool invocation).
    pub runs: Vec<SarifRun>,
}

/// A single run of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRun {
    /// Tool information.
    pub tool: SarifTool,
    /// List of results (findings).
    pub results: Vec<SarifResult>,
}

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifTool {
    /// Driver (the tool itself).
    pub driver: SarifDriver,
}

/// Tool driver information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifDriver {
    /// Tool name.
    pub name: String,
    /// Tool version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Information URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "informationUri")]
    pub information_uri: Option<String>,
}

/// A single result (finding).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifResult {
    /// Rule ID that triggered this result.
    #[serde(rename = "ruleId")]
    pub rule_id: String,
    /// Result level (note, warning, error).
    pub level: String,
    /// Human-readable message.
    pub message: SarifMessage,
    /// Locations where the issue was found.
    pub locations: Vec<SarifLocation>,
    /// Stable fingerprint for deduplication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprints: Option<SarifFingerprints>,
}

/// Message structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifMessage {
    /// Message text.
    pub text: String,
}

/// Location information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifLocation {
    /// Physical location in source code.
    #[serde(rename = "physicalLocation")]
    pub physical_location: SarifPhysicalLocation,
}

/// Physical location in source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifPhysicalLocation {
    /// Artifact (file) location.
    #[serde(rename = "artifactLocation")]
    pub artifact_location: SarifArtifactLocation,
    /// Region (line/column) information.
    pub region: SarifRegion,
}

/// Artifact location (file path).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    /// File URI or path.
    pub uri: String,
}

/// Region (line/column) information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRegion {
    /// Start line (1-indexed).
    #[serde(rename = "startLine")]
    pub start_line: usize,
}

/// Fingerprints for deduplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifFingerprints {
    /// Primary fingerprint (SHA-256 hash).
    #[serde(rename = "primaryLocationLineHash")]
    pub primary_location_line_hash: String,
}

impl From<Vec<Finding>> for SarifReport {
    fn from(findings: Vec<Finding>) -> Self {
        let results: Vec<SarifResult> = findings.into_iter().map(SarifResult::from).collect();

        SarifReport {
            version: "2.1.0".to_string(),
            schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "aptu-security-scanner".to_string(),
                        version: Some(env!("CARGO_PKG_VERSION").to_string()),
                        information_uri: Some("https://github.com/clouatre-labs/aptu".to_string()),
                    },
                },
                results,
            }],
        }
    }
}

impl From<Finding> for SarifResult {
    fn from(finding: Finding) -> Self {
        // Map severity to SARIF level
        let level = match finding.severity {
            super::types::Severity::Critical | super::types::Severity::High => "error",
            super::types::Severity::Medium => "warning",
            super::types::Severity::Low => "note",
        };

        // Generate stable fingerprint: hash of (file_path + line_number + pattern_id)
        let fingerprint_input = format!(
            "{}:{}:{}",
            finding.file_path, finding.line_number, finding.pattern_id
        );
        let mut hasher = Sha256::new();
        hasher.update(fingerprint_input.as_bytes());
        let hash = hasher.finalize();
        let fingerprint = format!("{hash:x}");

        SarifResult {
            rule_id: finding.pattern_id,
            level: level.to_string(),
            message: SarifMessage {
                text: finding.description,
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: finding.file_path,
                    },
                    region: SarifRegion {
                        start_line: finding.line_number,
                    },
                },
            }],
            fingerprints: Some(SarifFingerprints {
                primary_location_line_hash: fingerprint,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::types::{Confidence, Severity};

    #[test]
    fn test_sarif_report_structure() {
        let findings = vec![Finding {
            pattern_id: "hardcoded-secret".to_string(),
            description: "Hardcoded API key detected".to_string(),
            severity: Severity::Critical,
            confidence: Confidence::High,
            file_path: "src/config.rs".to_string(),
            line_number: 42,
            matched_text: "api_key = \"sk-1234567890\"".to_string(),
            cwe: Some("CWE-798".to_string()),
        }];

        let report = SarifReport::from(findings);

        assert_eq!(report.version, "2.1.0");
        assert_eq!(report.runs.len(), 1);
        assert_eq!(report.runs[0].results.len(), 1);
        assert_eq!(report.runs[0].tool.driver.name, "aptu-security-scanner");
    }

    #[test]
    fn test_severity_mapping() {
        let critical = Finding {
            pattern_id: "test".to_string(),
            description: "Test".to_string(),
            severity: Severity::Critical,
            confidence: Confidence::High,
            file_path: "test.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };

        let result = SarifResult::from(critical.clone());
        assert_eq!(result.level, "error");

        let medium = Finding {
            severity: Severity::Medium,
            ..critical.clone()
        };
        let result = SarifResult::from(medium);
        assert_eq!(result.level, "warning");

        let low = Finding {
            severity: Severity::Low,
            ..critical
        };
        let result = SarifResult::from(low);
        assert_eq!(result.level, "note");
    }

    #[test]
    fn test_fingerprint_stability() {
        let finding = Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test finding".to_string(),
            severity: Severity::High,
            confidence: Confidence::Medium,
            file_path: "src/main.rs".to_string(),
            line_number: 10,
            matched_text: "test code".to_string(),
            cwe: None,
        };

        let result1 = SarifResult::from(finding.clone());
        let result2 = SarifResult::from(finding);

        assert_eq!(
            result1
                .fingerprints
                .as_ref()
                .unwrap()
                .primary_location_line_hash,
            result2
                .fingerprints
                .as_ref()
                .unwrap()
                .primary_location_line_hash
        );
    }

    #[test]
    fn test_fingerprint_uniqueness() {
        let finding1 = Finding {
            pattern_id: "pattern1".to_string(),
            description: "Test".to_string(),
            severity: Severity::High,
            confidence: Confidence::High,
            file_path: "src/main.rs".to_string(),
            line_number: 10,
            matched_text: "test".to_string(),
            cwe: None,
        };

        let finding2 = Finding {
            pattern_id: "pattern2".to_string(),
            ..finding1.clone()
        };

        let result1 = SarifResult::from(finding1);
        let result2 = SarifResult::from(finding2);

        assert_ne!(
            result1
                .fingerprints
                .as_ref()
                .unwrap()
                .primary_location_line_hash,
            result2
                .fingerprints
                .as_ref()
                .unwrap()
                .primary_location_line_hash
        );
    }

    #[test]
    fn test_sarif_serialization() {
        let findings = vec![Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test finding".to_string(),
            severity: Severity::High,
            confidence: Confidence::Medium,
            file_path: "src/test.rs".to_string(),
            line_number: 5,
            matched_text: "test".to_string(),
            cwe: Some("CWE-123".to_string()),
        }];

        let report = SarifReport::from(findings);
        let json = serde_json::to_string(&report).unwrap();

        assert!(json.contains("\"version\":\"2.1.0\""));
        assert!(json.contains("\"ruleId\":\"test-pattern\""));
        assert!(json.contains("\"level\":\"error\""));
    }
}
