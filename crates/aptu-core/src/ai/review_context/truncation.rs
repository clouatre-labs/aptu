// SPDX-License-Identifier: Apache-2.0

//! Content truncation helpers for review context and patches.

/// Truncates `content` to at most `max_chars` characters, landing on the last newline
/// before the limit. Falls back to a char-boundary slice if no newline is found.
///
/// Returns the original content unchanged when its character count is already
/// within the limit.
#[must_use]
pub(crate) fn truncate_at_line_boundary(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }

    // Find the byte index of the max_chars-th character.
    let cutoff_byte = content
        .char_indices()
        .nth(max_chars)
        .map_or(content.len(), |(i, _)| i);

    // Scan backward from the cutoff byte to find the last newline.
    let truncated = &content[..cutoff_byte];
    if let Some(newline_pos) = truncated.rfind('\n') {
        content[..=newline_pos].to_string()
    } else {
        // No newline found; fall back to char-boundary slice at max_chars.
        content[..cutoff_byte].to_string()
    }
}
