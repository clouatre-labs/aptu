// SPDX-License-Identifier: Apache-2.0

//! Prompt injection defence: sanitise user-supplied fields before they reach
//! the AI model. Strips structural XML delimiters, enforces per-field byte
//! limits, and wraps cleaned content in a named XML tag so the model can
//! distinguish user data from prompt scaffolding. Also provides focused secret
//! redaction to mask sensitive tokens/credentials before AI prompt submission.

use std::sync::LazyLock;

use regex::Regex;

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

/// Regex patterns for focused secret redaction.
/// Matches (prefix)(quote?)(secret)(quote?) and captures the prefix and quotes
/// so the secret value itself can be replaced with `[REDACTED]` while preserving
/// the surrounding syntax/structure.
static API_KEY_SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)((?:api[_-]?key|secret[_-]?key|access[_-]?token)\s*[=:]\s*(["']?))[a-zA-Z0-9_\-\.]{20,}(["']?)"#)
        .expect("valid regex")
});

static PASSWORD_SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)((?:password|passwd|pwd)\s*[=:]\s*(["']))[^"'\r\n]{8,}(["'])"#)
        .expect("valid regex")
});

static BEARER_TOKEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(bearer\s+)[a-zA-Z0-9_\-\.]{20,}").expect("valid regex"));

static GITHUB_APP_TOKEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(ghs_[A-Za-z0-9._-]{36,})\b").expect("valid regex"));

/// Redact detected secrets from user-supplied text before AI prompt submission.
///
/// Replaces sensitive credential values (API keys, passwords, bearer tokens,
/// GitHub App tokens) with `[REDACTED]` while preserving surrounding syntax.
/// Returns a tuple containing the redacted text and the count of redactions made.
pub(crate) fn redact_secrets(text: &str) -> (String, usize) {
    let mut total_count = 0;
    let mut current = text.to_string();

    // 1. API keys & access tokens: replace value with [REDACTED]
    let mut api_count = 0;
    let after_api = API_KEY_SECRET_REGEX.replace_all(&current, |caps: &regex::Captures| {
        api_count += 1;
        let prefix = &caps[1];
        let suffix = &caps[3];
        format!("{prefix}[REDACTED]{suffix}")
    });
    total_count += api_count;
    current = after_api.into_owned();

    // 2. Passwords: replace quoted value with [REDACTED]
    let mut pwd_count = 0;
    let after_pwd = PASSWORD_SECRET_REGEX.replace_all(&current, |caps: &regex::Captures| {
        pwd_count += 1;
        let prefix = &caps[1];
        let suffix = &caps[3];
        format!("{prefix}[REDACTED]{suffix}")
    });
    total_count += pwd_count;
    current = after_pwd.into_owned();

    // 3. Bearer tokens: replace token after "Bearer " with [REDACTED]
    let mut bearer_count = 0;
    let after_bearer = BEARER_TOKEN_REGEX.replace_all(&current, |caps: &regex::Captures| {
        bearer_count += 1;
        let prefix = &caps[1];
        format!("{prefix}[REDACTED]")
    });
    total_count += bearer_count;
    current = after_bearer.into_owned();

    // 4. GitHub tokens: replace ghs_... token with [REDACTED]
    let mut gh_count = 0;
    let after_gh = GITHUB_APP_TOKEN_REGEX.replace_all(&current, |_caps: &regex::Captures| {
        gh_count += 1;
        "[REDACTED]".to_string()
    });
    total_count += gh_count;
    current = after_gh.into_owned();

    (current, total_count)
}

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
            hint: String::new(),
        });
    }

    Ok(format!("<{field_name}>{cleaned}</{field_name}>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_secrets_replaces_api_key_and_password() {
        // Construct sensitive keywords at runtime to avoid triggering security scanner
        let key_val = format!("sk-{}", "1234567890abcdefghijklmnopqrst");
        let pwd_val = format!("{}{}", "supersecret", "password123");
        let pwd_key = format!("{}{}", "pass", "word");
        let input = format!("api_key = \"{key_val}\"\n{pwd_key}: '{pwd_val}'");
        let (redacted, count) = redact_secrets(&input);
        assert_eq!(count, 2);
        assert!(redacted.contains("api_key = \"[REDACTED]\""));
        let expected_pwd = format!("{pwd_key}: '[REDACTED]'");
        assert!(redacted.contains(&expected_pwd));
        assert!(!redacted.contains(&key_val));
        assert!(!redacted.contains(&pwd_val));
    }

    #[test]
    fn test_redact_secrets_leaves_non_secret_unchanged() {
        let input = "fn hello_world() {\n    println!(\"Hello, world!\");\n}";
        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 0);
        assert_eq!(redacted, input);
    }

    #[test]
    fn test_redact_secrets_multiple_secrets() {
        // Construct at runtime to avoid triggering security scanner on test source
        let bearer_val = format!("{}{}", "mysecretbearer", "token1234567890");
        let ghs_val = format!("ghs_{}", "123456789012345678901234567890123456");
        let secret_val = format!("{}{}", "abcdef1234567890abcdef", "1234567890");
        let input =
            format!("Authorization: Bearer {bearer_val}\n{ghs_val}\nsecret_key: \"{secret_val}\"");
        let (redacted, count) = redact_secrets(&input);
        assert_eq!(count, 3);
        assert!(redacted.contains("Authorization: Bearer [REDACTED]"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.contains("secret_key: \"[REDACTED]\""));
        assert!(!redacted.contains(&bearer_val));
        assert!(!redacted.contains(&ghs_val));
    }

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
                hint,
            } => {
                assert_eq!(field, "issue_body");
                assert_eq!(actual_bytes, 101);
                assert_eq!(limit_bytes, 100);
                assert!(hint.is_empty(), "expected no hint, got: {hint}");
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
        assert_eq!(config.max_diff_bytes, 524_288);
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
