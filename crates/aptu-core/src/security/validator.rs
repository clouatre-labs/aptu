// SPDX-License-Identifier: Apache-2.0

//! LLM-based validation for security findings.
//!
//! Provides batched validation of security findings using AI to reduce false positives.
//! Batches 3-5 findings per LLM call for efficiency, with fallback to pattern confidence
//! on parsing errors.

use anyhow::{Context, Result};
use tracing::instrument;

use super::types::{Finding, ValidatedFinding, ValidationResult};
use crate::ai::client::AiClient;
use crate::ai::provider::AiProvider;
use crate::ai::types::{ChatCompletionRequest, ChatMessage, ResponseFormat};

/// Maximum lines of context to extract around a finding.
const CONTEXT_LINES: usize = 10;

/// Internal response structure for LLM validation.
#[derive(serde::Deserialize)]
struct ValidationResponse {
    results: Vec<ValidationResult>,
}

/// Security finding validator using LLM.
///
/// Validates security findings in batches to reduce false positives.
/// Falls back to pattern confidence if LLM validation fails.
#[derive(Debug)]
pub struct SecurityValidator {
    /// AI client for LLM calls.
    ai_client: AiClient,
}

impl SecurityValidator {
    /// Creates a new security validator.
    ///
    /// # Arguments
    ///
    /// * `ai_client` - AI client configured for validation
    pub fn new(ai_client: AiClient) -> Self {
        Self { ai_client }
    }

    /// Validates a batch of security findings using LLM.
    ///
    /// Sends up to `BATCH_SIZE` findings to the LLM for validation.
    /// Falls back to pattern confidence if LLM response is malformed.
    ///
    /// # Arguments
    ///
    /// * `findings` - Security findings to validate
    /// * `file_contents` - Map of file paths to their contents for context extraction
    ///
    /// # Returns
    ///
    /// Vector of validated findings with LLM reasoning
    #[instrument(skip(self, findings, file_contents), fields(count = findings.len()))]
    pub async fn validate_findings_batch(
        &self,
        findings: &[Finding],
        file_contents: &std::collections::HashMap<String, String>,
    ) -> Result<Vec<ValidatedFinding>> {
        if findings.is_empty() {
            return Ok(Vec::new());
        }

        // Build validation prompt
        let prompt = Self::build_batch_validation_prompt(findings, file_contents);

        // Build request
        let request = ChatCompletionRequest {
            model: self.ai_client.model().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Self::build_system_prompt(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            max_tokens: Some(self.ai_client.max_tokens()),
            temperature: Some(0.3),
        };

        // Send request and parse response
        match self.send_and_parse(&request).await {
            Ok(results) => {
                // Map results to validated findings
                let mut validated = Vec::new();
                for (i, finding) in findings.iter().enumerate() {
                    if let Some(result) = results.iter().find(|r| r.index == i) {
                        validated.push(ValidatedFinding {
                            finding: finding.clone(),
                            is_valid: result.is_valid,
                            reasoning: result.reasoning.clone(),
                            model_version: Some(self.ai_client.model().to_string()),
                        });
                    } else {
                        // Fallback: use pattern confidence
                        validated.push(Self::fallback_validation(finding));
                    }
                }
                Ok(validated)
            }
            Err(e) => {
                // Fallback: use pattern confidence for all findings
                tracing::warn!(error = %e, "LLM validation failed, using pattern confidence");
                Ok(findings.iter().map(Self::fallback_validation).collect())
            }
        }
    }

    /// Builds the system prompt for validation.
    fn build_system_prompt() -> String {
        r#"You are a security code reviewer. Analyze the provided security findings and determine if they are real vulnerabilities or false positives.

Your response MUST be valid JSON with this exact schema:
{
  "results": [
    {
      "index": 0,
      "is_valid": true,
      "reasoning": "Brief explanation of why this is/isn't a real issue"
    }
  ]
}

Guidelines:
- index: The 0-based index of the finding in the batch
- is_valid: true if this is a real security issue, false if it's a false positive
- reasoning: 1-2 sentence explanation of your decision

Consider:
1. Context: Is the code actually vulnerable in its usage context?
2. False positives: Test data, comments, documentation, or safe patterns?
3. Severity: Does the finding match the claimed severity?
4. Mitigation: Are there compensating controls in place?

Be conservative: when in doubt, mark as valid to avoid missing real issues."#
            .to_string()
    }

    /// Builds the validation prompt for a batch of findings.
    fn build_batch_validation_prompt(
        findings: &[Finding],
        file_contents: &std::collections::HashMap<String, String>,
    ) -> String {
        use std::fmt::Write;

        let mut prompt = String::new();
        prompt.push_str("Analyze these security findings:\n\n");

        for (i, finding) in findings.iter().enumerate() {
            let _ = writeln!(prompt, "Finding {i}:");
            let _ = writeln!(prompt, "  Pattern: {}", finding.pattern_id);
            let _ = writeln!(prompt, "  Description: {}", finding.description);
            let _ = writeln!(
                prompt,
                "  Severity: {:?}, Confidence: {:?}",
                finding.severity, finding.confidence
            );
            let _ = writeln!(
                prompt,
                "  File: {}:{}",
                finding.file_path, finding.line_number
            );
            let _ = writeln!(prompt, "  Matched: {}", finding.matched_text);

            // Extract context snippet
            if let Some(snippet) =
                extract_snippet(file_contents.get(&finding.file_path), finding.line_number)
            {
                let _ = writeln!(prompt, "  Context:\n{snippet}");
            }

            prompt.push('\n');
        }

        prompt
    }

    /// Sends a validation request and parses the response.
    async fn send_and_parse(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<Vec<ValidationResult>> {
        // Send request using AiProvider trait
        let completion = self.ai_client.send_request_inner(request).await?;

        // Extract message content
        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .context("No response from AI model")?;

        // Parse JSON response
        let response: ValidationResponse = serde_json::from_str(&content)
            .context("Failed to parse validation response as JSON")?;

        Ok(response.results)
    }

    /// Creates a fallback validated finding using pattern confidence.
    fn fallback_validation(finding: &Finding) -> ValidatedFinding {
        use super::types::Confidence;

        let is_valid = matches!(finding.confidence, Confidence::High | Confidence::Medium);
        let reasoning = format!(
            "LLM validation unavailable, using pattern confidence: {:?}",
            finding.confidence
        );

        ValidatedFinding {
            finding: finding.clone(),
            is_valid,
            reasoning,
            model_version: None,
        }
    }
}

/// Extracts a code snippet with context around a line number.
///
/// # Arguments
///
/// * `content` - File content
/// * `line_number` - Target line number (1-indexed)
///
/// # Returns
///
/// Code snippet with up to `CONTEXT_LINES` before and after the target line
fn extract_snippet(content: Option<&String>, line_number: usize) -> Option<String> {
    use std::fmt::Write;

    let content = content?;
    let lines: Vec<&str> = content.lines().collect();

    if line_number == 0 || line_number > lines.len() {
        return None;
    }

    // Calculate range (1-indexed to 0-indexed)
    let target_idx = line_number - 1;
    let start = target_idx.saturating_sub(CONTEXT_LINES);
    let end = (target_idx + CONTEXT_LINES + 1).min(lines.len());

    let mut snippet = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        let line_num = start + i + 1;
        let marker = if line_num == line_number { ">" } else { " " };
        let _ = writeln!(snippet, "{marker} {line_num:4} | {line}");
    }

    Some(snippet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::types::{Confidence, Severity};

    #[test]
    fn test_extract_snippet_with_context() {
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\n".to_string();
        let snippet = extract_snippet(Some(&content), 4);

        assert!(snippet.is_some());
        let snippet = snippet.unwrap();
        assert!(snippet.contains(">    4 | line 4"));
        assert!(snippet.contains("     1 | line 1"));
        assert!(snippet.contains("     7 | line 7"));
    }

    #[test]
    fn test_extract_snippet_at_start() {
        let content = "line 1\nline 2\nline 3\n".to_string();
        let snippet = extract_snippet(Some(&content), 1);

        assert!(snippet.is_some());
        let snippet = snippet.unwrap();
        assert!(snippet.contains(">    1 | line 1"));
        assert!(!snippet.contains("     0 |"));
    }

    #[test]
    fn test_extract_snippet_at_end() {
        let content = "line 1\nline 2\nline 3\n".to_string();
        let snippet = extract_snippet(Some(&content), 3);

        assert!(snippet.is_some());
        let snippet = snippet.unwrap();
        assert!(snippet.contains(">    3 | line 3"));
    }

    #[test]
    fn test_extract_snippet_invalid_line() {
        let content = "line 1\nline 2\n".to_string();
        let snippet = extract_snippet(Some(&content), 10);

        assert!(snippet.is_none());
    }

    #[test]
    fn test_fallback_validation_high_confidence() {
        let finding = Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test finding".to_string(),
            severity: Severity::High,
            confidence: Confidence::High,
            file_path: "test.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };

        let validated = SecurityValidator::fallback_validation(&finding);
        assert!(validated.is_valid);
        assert!(validated.reasoning.contains("High"));
    }

    #[test]
    fn test_fallback_validation_low_confidence() {
        let finding = Finding {
            pattern_id: "test-pattern".to_string(),
            description: "Test finding".to_string(),
            severity: Severity::High,
            confidence: Confidence::Low,
            file_path: "test.rs".to_string(),
            line_number: 1,
            matched_text: "test".to_string(),
            cwe: None,
        };

        let validated = SecurityValidator::fallback_validation(&finding);
        assert!(!validated.is_valid);
        assert!(validated.reasoning.contains("Low"));
    }

    #[test]
    fn test_build_system_prompt() {
        let prompt = SecurityValidator::build_system_prompt();
        assert!(prompt.contains("security code reviewer"));
        assert!(prompt.contains("\"results\""));
        assert!(prompt.contains("\"index\""));
        assert!(prompt.contains("\"is_valid\""));
        assert!(prompt.contains("\"reasoning\""));
    }

    #[test]
    fn test_parse_validation_response() {
        let json = r#"{
            "results": [
                {
                    "index": 0,
                    "is_valid": true,
                    "reasoning": "This is a real vulnerability"
                },
                {
                    "index": 1,
                    "is_valid": false,
                    "reasoning": "This is test data"
                }
            ]
        }"#;

        let response: ValidationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].index, 0);
        assert!(response.results[0].is_valid);
        assert_eq!(response.results[1].index, 1);
        assert!(!response.results[1].is_valid);
    }
}
