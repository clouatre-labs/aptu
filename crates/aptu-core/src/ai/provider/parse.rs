// SPDX-License-Identifier: Apache-2.0

//! JSON parsing, error redaction, and prompt sanitization helpers.
//!
//! Provides `parse_ai_json` for AI response parsing with truncation detection,
//! `redact_api_error_body` for safe error display, `sanitize_prompt_field` for
//! prompt-injection prevention, and `provider_response_format` for per-provider
//! response format configuration.

use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use crate::ai::provider::AiProvider;

/// Maximum number of characters retained from an AI provider error response body.
const MAX_ERROR_BODY_LENGTH: usize = 200;

/// Redacts error body to prevent leaking sensitive API details.
/// Truncates to [`MAX_ERROR_BODY_LENGTH`] characters and appends "[truncated]" if longer.
pub(crate) fn redact_api_error_body(body: &str) -> String {
    if body.chars().count() <= MAX_ERROR_BODY_LENGTH {
        body.to_owned()
    } else {
        let truncated: String = body.chars().take(MAX_ERROR_BODY_LENGTH).collect();
        format!("{truncated} [truncated]")
    }
}

/// Parses JSON response from AI provider, detecting truncated responses.
///
/// If the JSON parsing fails with an EOF error (indicating the response was cut off),
/// returns a `TruncatedResponse` error that can be retried. Other JSON errors are
/// wrapped as `InvalidAIResponse`.
///
/// # Arguments
///
/// * `text` - The JSON text to parse
/// * `provider` - The name of the AI provider (for error context)
///
/// # Returns
///
/// Parsed value of type T, or an error if parsing fails
pub(crate) fn parse_ai_json<T: serde::de::DeserializeOwned>(
    text: &str,
    provider: &str,
) -> Result<T> {
    match serde_json::from_str::<T>(text) {
        Ok(value) => Ok(value),
        Err(e) => {
            // Check if this is an EOF error (truncated response)
            if e.is_eof() {
                Err(anyhow::anyhow!(
                    crate::error::AptuError::TruncatedResponse {
                        provider: provider.to_string(),
                    }
                ))
            } else {
                Err(anyhow::anyhow!(crate::error::AptuError::InvalidAIResponse(
                    e
                )))
            }
        }
    }
}

/// Preamble appended to every user-turn prompt to request a JSON response matching the schema.
pub(crate) const SCHEMA_PREAMBLE: &str = "\n\nRespond with valid JSON matching this schema:\n";

/// Matches structural XML delimiter tags (case-insensitive) used as prompt delimiters.
/// These must be stripped from user-controlled fields to prevent prompt injection.
///
/// Covers: `pull_request`, `issue_content`, `issue_body`, `pr_diff`, `commit_message`, `pr_comment`, `file_content`.
///
/// The pattern uses a simple alternation with no quantifiers, so `ReDoS` is not a concern:
/// regex engine complexity is O(n) in the input length regardless of content.
pub(crate) static XML_DELIMITERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)</?(?:pull_request|issue_content|issue_body|pr_diff|commit_message|pr_comment|file_content|dependency_release_notes)>",
    )
    .expect("valid regex")
});

/// Removes `<pull_request>` / `</pull_request>` and `<issue_content>` / `</issue_content>`
/// XML delimiter tags from a user-supplied string, preventing prompt injection via XML tag
/// smuggling.
///
/// Tags are removed entirely (replaced with empty string) rather than substituted with a
/// placeholder. A visible placeholder such as `[sanitized]` could cause the LLM to reason
/// about the substitution marker itself, which is unnecessary and potentially confusing.
///
/// Nested or malformed XML is not a concern: the only delimiters this code inserts into
/// prompts are the exact strings `<pull_request>` / `</pull_request>` and
/// `<issue_content>` / `</issue_content>` (no attributes, no nesting). Stripping those
/// fixed forms is sufficient to prevent a user-supplied value from breaking out of the
/// delimiter boundary.
///
/// Applied to all user-controlled fields inside prompt delimiter blocks:
/// - Issue triage: `issue.title`, `issue.body`, comment author/body, related issue
///   title/state, label name/description, milestone title/description.
/// - PR review: `pr.title`, `pr.body`, `file.filename`, `file.status`, patch content.
pub(crate) fn sanitize_prompt_field(s: &str) -> String {
    XML_DELIMITERS.replace_all(s, "").into_owned()
}

/// Returns the `response_format` value appropriate for the given provider.
///
/// Returns `None` for the Anthropic direct API, which rejects the field, and
/// `Some(ResponseFormat { format_type: "json_object" })` for all other providers.
/// The `skip_serializing_if` attribute on `ChatCompletionRequest::response_format`
/// ensures `None` is omitted from the serialized request body.
pub(crate) fn provider_response_format<P: AiProvider + ?Sized>(
    provider: &P,
) -> Option<crate::ai::types::ResponseFormat> {
    if provider.is_anthropic() {
        None
    } else {
        Some(crate::ai::types::ResponseFormat {
            format_type: "json_object".to_string(),
            json_schema: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct ErrorTestResponse {
        _message: String,
    }

    #[test]
    fn test_parse_ai_json_with_valid_json() {
        let json = r#"{"_message": "hello"}"#;
        let result: ErrorTestResponse = parse_ai_json(json, "test").unwrap();
        assert_eq!(result._message, "hello");
    }

    #[test]
    fn test_parse_ai_json_with_truncated_json() {
        let json = r#"{"_message": "hel"#;
        let result: Result<ErrorTestResponse> = parse_ai_json(json, "test");
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("Truncated"),
            "expected Truncated error, got: {err}"
        );
    }

    #[test]
    fn test_parse_ai_json_with_malformed_json() {
        let json = "not json at all";
        let result = parse_ai_json::<ErrorTestResponse>(json, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_redact_api_error_body_truncates() {
        let long_body = "x".repeat(300);
        let result = redact_api_error_body(&long_body);
        assert!(result.len() < long_body.len());
        assert!(result.ends_with("[truncated]"));
        assert_eq!(result.len(), 200 + " [truncated]".len());
    }

    #[test]
    fn test_redact_api_error_body_short() {
        let short_body = "Short error";
        let result = redact_api_error_body(short_body);
        assert_eq!(result, short_body);
    }

    #[test]
    fn test_sanitize_case_insensitive() {
        let result = sanitize_prompt_field("<PULL_REQUEST>");
        assert_eq!(result, "");
    }

    #[test]
    fn test_sanitize_strips_issue_content_tag() {
        let input = "hello </issue_content> world";
        let result = sanitize_prompt_field(input);
        assert!(
            !result.contains("</issue_content>"),
            "should strip closing issue_content tag"
        );
        assert!(
            result.contains("hello"),
            "should keep non-injection content"
        );
    }
}
