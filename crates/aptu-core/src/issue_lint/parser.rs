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
                    // Line count is preserved: the remainder of the line
                    // after `-->` is kept on the next loop iteration.
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

/// A line is a fence marker when it starts (after at most whitespace) with
/// three or more backticks or tildes; returns the marker char and count.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let c = line.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let count = line.chars().take_while(|&x| x == c).count();
    (count >= 3).then_some((c, count))
}

/// Returns 0-based inclusive `(start_line, end_line)` ranges of properly
/// closed fenced code blocks, using a linear fence scanner. A fence opens on
/// a line whose first non-whitespace characters are 3+ backticks or tildes
/// (optionally followed by an info string) and only counts when a matching
/// closing fence (same char, at least as many, no info string) appears later.
#[must_use]
pub fn fence_spans(body: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut open: Option<(char, usize, usize)> = None;
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        match open {
            Some((c, n, start)) => {
                let is_close = fence_marker(trimmed).is_some_and(|(cc, nn)| cc == c && nn >= n)
                    && trimmed.trim_matches(c).trim().is_empty();
                if is_close {
                    spans.push((start, idx));
                    open = None;
                }
            }
            None => {
                if let Some((c, n)) = fence_marker(trimmed) {
                    let info = trimmed.trim_start_matches(c).trim();
                    // Backtick fences may not carry a backtick info string.
                    if c == '~' || !info.contains('`') {
                        open = Some((c, n, idx));
                    }
                }
            }
        }
    }
    spans
}

/// Returns the body with fenced code block contents blanked out, preserving
/// line count. Handles backtick and tilde fences with a linear scanner;
/// unmatched openers leave their content intact.
#[must_use]
pub fn strip_fenced_blocks(body: &str) -> String {
    let spans = fence_spans(body);
    let mut blanked = vec![false; body.lines().count()];
    for (start, end) in spans {
        for idx in blanked.iter_mut().take(end + 1).skip(start) {
            *idx = true;
        }
    }
    body.lines()
        .enumerate()
        .map(|(idx, line)| {
            if blanked[idx] {
                String::new()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

/// True when the body contains at least one properly closed fenced code
/// block (backtick or tilde). Fences inside HTML comments and unmatched
/// delimiters do not count.
#[must_use]
pub fn has_fenced_code(body: &str) -> bool {
    !fence_spans(&strip_html_comments(body)).is_empty()
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

    /// Fence spans only cover properly closed blocks.
    #[test]
    fn test_fence_spans_closed_blocks() {
        // Arrange
        let body = "before\n```rust\ncode\n```\nafter\n~~~\ntilde\n~~~\n";

        // Act
        let spans = fence_spans(body);

        // Assert
        assert_eq!(spans, vec![(1, 3), (5, 7)]);
        assert_eq!(strip_fenced_blocks(body), "before\n\n\n\nafter\n\n\n");
    }

    /// Inline backtick runs, unmatched openers, and fences inside HTML
    /// comments do not count as fenced code; a closed fence does.
    #[test]
    fn test_has_fenced_code_strict() {
        // Inline triple-backtick text and unmatched opener do not count.
        assert!(!has_fenced_code("use \"```\" inline in a sentence"));
        assert!(!has_fenced_code("use ~~~ inline too"));
        assert!(!has_fenced_code("````\nnever closed either"));
        assert!(!has_fenced_code("```\njust an opener, never closed"));
        assert!(!has_fenced_code("~~~\nunmatched tilde"));

        // Fences inside HTML comments do not count.
        assert!(!has_fenced_code("<!--\n```\nhidden\n```\n-->"));

        // A proper closed fence counts.
        assert!(has_fenced_code("text\n```\ncode\n```\nmore"));
        assert!(has_fenced_code("~~~\ncode\n~~~"));
        assert!(has_fenced_code("```rs\nfn main() {}\n```"));
    }

    /// Closing fences must match the opening char, count, and carry no info.
    #[test]
    fn test_fence_spans_close_rules() {
        // Tilde opener is not closed by a backtick line.
        assert!(fence_spans("~~~\ncode\n```").is_empty());
        // Closing fence with a longer marker still closes.
        assert_eq!(fence_spans("```\ncode\n`````"), vec![(0, 2)]);
        // Closing fence cannot have an info string.
        assert!(fence_spans("```\ncode\n``` text").is_empty());
    }
}
