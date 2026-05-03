// SPDX-License-Identifier: Apache-2.0

//! Prompt injection defence: sanitise user-supplied fields before they reach
//! the AI model. Strips structural XML delimiters, enforces per-field byte
//! limits, and wraps cleaned content in a named XML tag so the model can
//! distinguish user data from prompt scaffolding.

use crate::error::AptuError;

/// All structural XML delimiters that must be stripped from user input.
///
/// These are the tag names (opening and closing) used by the prompt
/// scaffolding. Any occurrence in user-supplied data would allow an
/// attacker to break out of the intended data section.
const STRUCTURAL_TAGS: &[&str] = &[
    "<pull_request>",
    "</pull_request>",
    "<issue_content>",
    "</issue_content>",
    "<issue_body>",
    "</issue_body>",
    "<pr_diff>",
    "</pr_diff>",
    "<commit_message>",
    "</commit_message>",
    "<pr_comment>",
    "</pr_comment>",
    "<file_content>",
    "</file_content>",
];

/// Sanitise a single user-supplied field for safe inclusion in an AI prompt.
///
/// 1. Strips all structural XML delimiters listed in `STRUCTURAL_TAGS`.
/// 2. Enforces `max_bytes`: returns [`AptuError::InputExceedsLimit`] if the
///    cleaned content (in bytes) exceeds the limit.
/// 3. Wraps the cleaned content in `<{field_name}>…</{field_name}>` tags so
///    the model can identify the provenance of the data.
///
/// # Errors
///
/// Returns [`AptuError::InputExceedsLimit`] when the sanitised content exceeds
/// `max_bytes`.
pub(crate) fn sanitise_user_field(
    field_name: &str,
    input: &str,
    max_bytes: usize,
) -> Result<String, AptuError> {
    // Strip structural delimiters.
    let mut cleaned = input.to_owned();
    for tag in STRUCTURAL_TAGS {
        cleaned = cleaned.replace(tag, "");
    }

    let actual_bytes = cleaned.len();
    if actual_bytes > max_bytes {
        return Err(AptuError::InputExceedsLimit {
            field: field_name.to_owned(),
            actual_bytes,
            limit_bytes: max_bytes,
        });
    }

    Ok(format!("<{field_name}>{cleaned}</{field_name}>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitise_strips_structural_delimiters() {
        let input = "before <pull_request> middle </pull_request> after";
        let result = sanitise_user_field("issue_body", input, 1024).unwrap();
        // Structural delimiters from input should be stripped
        assert!(!result.contains("<pull_request>"));
        assert!(!result.contains("</pull_request>"));
        // But the output should be wrapped in the field name tags
        assert!(result.starts_with("<issue_body>"));
        assert!(result.ends_with("</issue_body>"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    #[test]
    fn test_sanitise_wraps_in_named_xml_tag() {
        let input = "clean content";
        let result = sanitise_user_field("pr_diff", input, 1024).unwrap();
        assert!(result.starts_with("<pr_diff>"));
        assert!(result.ends_with("</pr_diff>"));
    }

    #[test]
    fn test_sanitise_byte_limit_exceeded_returns_error() {
        let input = "a".repeat(101);
        let err = sanitise_user_field("issue_body", &input, 100).unwrap_err();
        match err {
            AptuError::InputExceedsLimit {
                field,
                actual_bytes,
                limit_bytes,
            } => {
                assert_eq!(field, "issue_body");
                assert_eq!(actual_bytes, 101);
                assert_eq!(limit_bytes, 100);
            }
            other => panic!("expected InputExceedsLimit, got {other:?}"),
        }
    }

    #[test]
    fn test_sanitise_within_limit_succeeds() {
        let input = "hello world";
        let result = sanitise_user_field("commit_message", input, 100).unwrap();
        assert!(result.contains("hello world"));
    }

    #[test]
    fn test_sanitise_empty_input() {
        let result = sanitise_user_field("issue_body", "", 1024).unwrap();
        assert_eq!(result, "<issue_body></issue_body>");
    }

    #[test]
    fn test_sanitise_only_tags_becomes_empty() {
        let input = "<pull_request></pull_request><issue_content></issue_content>";
        let result = sanitise_user_field("issue_body", input, 1024).unwrap();
        assert_eq!(result, "<issue_body></issue_body>");
    }

    #[test]
    fn test_prompt_config_defaults() {
        let config = crate::config::PromptConfig::default();
        assert_eq!(config.max_issue_body_bytes, 32_768);
        assert_eq!(config.max_diff_bytes, 131_072);
        assert_eq!(config.max_commit_message_bytes, 4_096);
    }

    #[test]
    fn test_sanitise_multibyte_utf8_at_boundary() {
        // Each emoji is 4 bytes in UTF-8.
        // If max_bytes is set to 8, and we have "Hello " (6 bytes) + emoji (4 bytes) = 10 bytes,
        // the function must return an error rather than panic or truncate.
        let emoji_str = "Hello \u{1F600}"; // "Hello " (6 bytes) + emoji (4 bytes) = 10 bytes
        let err = sanitise_user_field("test_field", emoji_str, 8).unwrap_err();
        match err {
            AptuError::InputExceedsLimit {
                actual_bytes,
                limit_bytes,
                ..
            } => {
                assert_eq!(actual_bytes, 10);
                assert_eq!(limit_bytes, 8);
            }
            other => panic!("expected InputExceedsLimit, got {other:?}"),
        }
    }
}
