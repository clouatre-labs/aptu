// SPDX-License-Identifier: Apache-2.0

//! Repository instructions fetching for PR review context.
//!
//! Fetches AGENTS.md or .github/instructions/pr-review.md from a repository
//! to inject as context into PR review prompts.

use tracing::instrument;

/// Fetches repository instructions for PR review context.
///
/// Attempts to fetch instructions from the repository in the following order:
/// 1. If `override_path` is provided, fetch only from that path
/// 2. Otherwise, try "AGENTS.md" then ".github/instructions/pr-review.md"
///
/// Returns `None` if:
/// - Neither file exists (when no override)
/// - File content is empty
/// - Any error occurs during fetching
///
/// The returned content:
/// - Has YAML frontmatter stripped (leading `---\n...---\n` block)
/// - Is truncated to 1500 characters maximum
///
/// # Arguments
///
/// * `client` - Octocrab GitHub API client
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `head_sha` - Commit SHA to fetch from
/// * `override_path` - Optional path to fetch instead of default paths
#[cfg(not(target_arch = "wasm32"))]
#[instrument(skip(client), fields(owner = %owner, repo = %repo, head_sha = %head_sha))]
pub async fn fetch_repo_instructions(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    head_sha: &str,
    override_path: Option<&str>,
    max_chars: usize,
) -> Option<String> {
    let paths = if let Some(path) = override_path {
        vec![path.to_string()]
    } else {
        vec![
            "AGENTS.md".to_string(),
            ".github/instructions/pr-review.md".to_string(),
        ]
    };

    for path in paths {
        match fetch_file_content(client, owner, repo, &path, head_sha).await {
            Some(content) => {
                if !content.is_empty() {
                    let stripped = strip_yaml_frontmatter(&content);
                    let truncated = truncate_to_chars(&stripped, max_chars);
                    tracing::debug!(
                        file = %path,
                        chars = truncated.len(),
                        "Fetched repo instructions"
                    );
                    return Some(truncated);
                }
            }
            None => {
                tracing::debug!(file = %path, "Instructions file not found or error fetching");
            }
        }
    }

    tracing::debug!("No instructions file found");
    None
}

/// Fetches a single file's content from the repository.
///
/// Returns `None` on any error (404, decode failure, etc.).
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_file_content(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    filename: &str,
    head_sha: &str,
) -> Option<String> {
    match client
        .repos(owner, repo)
        .get_content()
        .path(filename)
        .r#ref(head_sha)
        .send()
        .await
    {
        Ok(content) => {
            // Try to decode the first item (should be the file, not a directory listing)
            if let Some(item) = content.items.first() {
                if let Some(decoded) = item.decoded_content() {
                    return Some(decoded);
                }
                tracing::debug!(
                    path = filename,
                    "failed to decode instructions file content"
                );
                return None;
            }
            None
        }
        Err(e) => {
            tracing::debug!(error = %e, path = filename, "failed to fetch instructions file");
            None
        }
    }
}

/// Strips YAML frontmatter from content.
///
/// If content starts with `---\n`, finds the closing `---\n` and removes that block.
/// Handles both LF (\n) and CRLF (\r\n) line endings.
/// Otherwise, returns content unchanged.
fn strip_yaml_frontmatter(content: &str) -> String {
    // Only strip if content begins with a frontmatter delimiter
    let after_open = if let Some(rest) = content.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = content.strip_prefix("---\r\n") {
        rest
    } else {
        return content.to_string();
    };

    // Find closing delimiter; if absent, return content as-is (no frontmatter)
    if let Some(end) = after_open.find("\n---\n") {
        after_open[end + 5..].to_string()
    } else if let Some(end) = after_open.find("\r\n---\r\n") {
        after_open[end + 7..].to_string()
    } else {
        // No closing delimiter found; treat entire content as body
        content.to_string()
    }
}

/// Truncates content to a maximum number of characters.
fn truncate_to_chars(content: &str, max_chars: usize) -> String {
    content.chars().take(max_chars).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_yaml_frontmatter_with_frontmatter() {
        let content = "---\ntitle: Test\nauthor: Me\n---\nActual content here";
        let result = strip_yaml_frontmatter(content);
        assert_eq!(result, "Actual content here");
    }

    #[test]
    fn test_strip_yaml_frontmatter_without_frontmatter() {
        let content = "Just plain content";
        let result = strip_yaml_frontmatter(content);
        assert_eq!(result, "Just plain content");
    }

    #[test]
    fn test_strip_yaml_frontmatter_no_closing() {
        let content = "---\ntitle: Test\nNo closing marker";
        let result = strip_yaml_frontmatter(content);
        // If no closing delimiter found, treat entire content as body (no frontmatter)
        assert_eq!(result, "---\ntitle: Test\nNo closing marker");
    }

    #[test]
    fn test_truncate_to_chars() {
        let content = "0123456789";
        let result = truncate_to_chars(content, 5);
        assert_eq!(result, "01234");
    }

    #[test]
    fn test_truncate_to_chars_longer_than_max() {
        let content = "short";
        let result = truncate_to_chars(content, 100);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_to_chars_unicode() {
        let content = "hello 🌍 world";
        let result = truncate_to_chars(content, 8);
        assert_eq!(result, "hello 🌍 ");
    }
}
