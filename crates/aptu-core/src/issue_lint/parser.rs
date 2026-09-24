// SPDX-License-Identifier: Apache-2.0

//! Markdown parsing helpers for issue-body linting.
//!
//! All functions are pure and operate on `&str`. Fenced code blocks are
//! replaced with blank lines so heading line numbers remain stable; HTML
//! comments are stripped entirely. CRLF line endings are handled by
//! trimming `\r` before matching.

/// Returns the body with HTML comments removed, preserving line count.
#[must_use]
pub fn strip_html_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_comment = false;
    for line in body.lines() {
        let mut rest = line;
        let mut kept = String::new();
        while !rest.is_empty() {
            if in_comment {
                if let Some(idx) = rest.find("-->") {
                    // Keep line count: emit blanks for the removed span.
                    for _ in 0..rest[..idx].matches('\n').count() {
                        kept.push('\n');
                    }
                    rest = &rest[idx + 3..];
                    in_comment = false;
                } else {
                    kept.push('\n');
                    rest = "";
                }
            } else if let Some(idx) = rest.find("<!--") {
                kept.push_str(&rest[..idx]);
                rest = &rest[idx + 4..];
                in_comment = true;
            } else {
                kept.push_str(rest);
                rest = "";
            }
        }
        out.push_str(kept.trim_end_matches('\n'));
        out.push('\n');
    }
    if body.ends_with('\n') {
        out
    } else {
        out.trim_end_matches('\n').to_string()
    }
}

/// Returns the body with fenced code block contents blanked out, preserving
/// line count. Handles backtick and tilde fences with a linear scanner.
#[must_use]
pub fn strip_fenced_blocks(body: &str) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    let mut fence_token: Option<String> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(token) = &fence_token {
            out_lines.push(String::new());
            if trimmed.starts_with(token.as_str()) {
                fence_token = None;
            }
        } else {
            let fence = ['`', '~']
                .into_iter()
                .find(|c| trimmed.starts_with(&c.to_string().repeat(3)));
            if let Some(c) = fence {
                fence_token = Some(c.to_string());
                out_lines.push(String::new());
            } else {
                out_lines.push(line.to_string());
            }
        }
    }
    out_lines.join("\n")
}

/// Extracts H2 headings (lines starting with `## `) with 1-based line
/// numbers. Blockquoted headings (`> ## `) are ignored. CRLF is tolerated.
#[must_use]
pub fn extract_h2_headings(body: &str) -> Vec<(String, usize)> {
    strip_fenced_blocks(&strip_html_comments(body))
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim_end_matches('\r');
            line.strip_prefix("## ")
                .map(|heading| (heading.trim().to_string(), idx + 1))
        })
        .collect()
}

/// True when the body contains a fenced code block (backtick or tilde).
/// Must run on the raw body: fences cannot be detected after stripping.
#[must_use]
pub fn has_fenced_code(body: &str) -> bool {
    body.contains("```") || body.contains("~~~")
}

/// True when the body references an external URL (`http://`/`https://`)
/// or another issue (`#N` with at least one digit).
#[must_use]
pub fn has_external_reference(body: &str) -> bool {
    let lower = body.to_lowercase();
    if lower.contains("http://") || lower.contains("https://") {
        return true;
    }
    body.contains('#')
        && body
            .split('#')
            .skip(1)
            .any(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

/// True when the stripped body contains a file-path-like reference (a token
/// containing `/` and a `.`), or when the raw body contains a code fence.
#[must_use]
pub fn has_code_example(body: &str) -> bool {
    if has_fenced_code(body) {
        return true;
    }
    strip_fenced_blocks(&strip_html_comments(body))
        .split_whitespace()
        .any(|token| {
            let token = token.trim_matches(|c: char| "()`*\"".contains(c));
            token.contains('/') && token.contains('.') && !token.contains("://")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H2 headings inside fenced code blocks and HTML comments are ignored.
    #[test]
    fn test_extract_h2_headings_ignores_fences_and_comments() {
        // Arrange
        let body = "## Real\n<!--\n## In Comment\n-->\n```markdown\n## In Fence\n```\n~~~\n## Tilde Fence\n~~~\n> ## Quoted\n";

        // Act
        let headings = extract_h2_headings(body);

        // Assert
        assert_eq!(headings, vec![("Real".to_string(), 1)]);
    }

    /// Line numbers stay stable after stripping, and CRLF is tolerated.
    #[test]
    fn test_extract_h2_headings_line_numbers_and_crlf() {
        // Arrange
        let body = "intro\r\n## Second\r\n```\n## In Fence\n```\r\n## Fourth\r\n";

        // Act
        let headings = extract_h2_headings(body);

        // Assert
        assert_eq!(
            headings,
            vec![("Second".to_string(), 2), ("Fourth".to_string(), 6),]
        );
    }

    /// Unclosed HTML comments remove the remainder of the body.
    #[test]
    fn test_unclosed_html_comment() {
        // Arrange
        let body = "## One\n<!-- unclosed\n## Hidden\n";

        // Act
        let headings = extract_h2_headings(body);

        // Assert
        assert_eq!(headings, vec![("One".to_string(), 1)]);
    }

    /// External-reference detection covers URLs and #N but not bare '#'.
    #[test]
    fn test_has_external_reference() {
        // Arrange / Act / Assert
        assert!(has_external_reference("see https://example.com/x"));
        assert!(has_external_reference("dup of #123"));
        assert!(!has_external_reference("no refs here"));
        assert!(!has_external_reference("only a #symbol"));
    }

    /// Code-example detection covers fences and file-path tokens.
    #[test]
    fn test_has_code_example() {
        // Arrange / Act / Assert
        assert!(has_code_example("```\ncode\n```"));
        assert!(has_code_example("see `src/main.rs`"));
        assert!(!has_code_example("plain text only"));
    }
}
