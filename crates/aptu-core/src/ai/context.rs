// SPDX-License-Identifier: Apache-2.0

//! Best practices context for AI system prompts.
//!
//! Provides current tooling recommendations to prevent AI from suggesting
//! outdated tools (mypy, bandit, eslint+prettier) and ensure modern
//! alternatives (ruff, pyright, biome, uv) are recommended.

/// Best practices context for 2026 tooling recommendations.
///
/// This context is injected into all system prompts to ensure AI provides
/// current, modern recommendations for Python, JavaScript/TypeScript, Rust,
/// and AI models.
pub const TOOLING_CONTEXT: &str = include_str!("prompts/tooling_context.md");

/// Loads custom guidance from configuration if available.
///
/// # Arguments
///
/// * `custom_guidance` - Optional custom guidance string from config
///
/// # Returns
///
/// A formatted string combining default [`TOOLING_CONTEXT`] with custom guidance,
/// or just [`TOOLING_CONTEXT`] if no custom guidance is provided.
#[must_use]
pub fn load_custom_guidance(custom_guidance: Option<&str>) -> String {
    match custom_guidance {
        Some(guidance) => format!("{TOOLING_CONTEXT}\n\n## Custom Guidance\n\n{guidance}"),
        None => TOOLING_CONTEXT.to_string(),
    }
}

/// Load a system prompt override from `~/.config/aptu/prompts/<name>.md`.
/// Returns the file content if the file exists and is readable, or `None` otherwise.
pub async fn load_system_prompt_override(name: &str) -> Option<String> {
    let path = crate::config::prompts_dir().join(format!("{name}.md"));
    tokio::fs::read_to_string(&path).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooling_context_contains_python_recommendations() {
        assert!(TOOLING_CONTEXT.contains("ruff"));
        assert!(TOOLING_CONTEXT.contains("pyright"));
        assert!(TOOLING_CONTEXT.contains("uv"));
    }

    #[test]
    fn test_tooling_context_contains_javascript_recommendations() {
        assert!(TOOLING_CONTEXT.contains("biome"));
        assert!(TOOLING_CONTEXT.contains("bun"));
        assert!(TOOLING_CONTEXT.contains("vitest"));
    }

    #[test]
    fn test_tooling_context_contains_rust_recommendations() {
        assert!(TOOLING_CONTEXT.contains("Cargo.toml"));
        assert!(TOOLING_CONTEXT.contains("iterators"));
    }

    #[test]
    fn test_tooling_context_contains_current_ai_models() {
        assert!(TOOLING_CONTEXT.contains("Sonnet/Opus 4.5"));
        assert!(TOOLING_CONTEXT.contains("GPT-5.2"));
        assert!(TOOLING_CONTEXT.contains("Gemini 3"));
        assert!(TOOLING_CONTEXT.contains("NOT 3.x"));
        assert!(TOOLING_CONTEXT.contains("NOT GPT-4"));
        assert!(TOOLING_CONTEXT.contains("NOT 2.x/1.x"));
    }

    #[test]
    fn test_load_custom_guidance_without_custom() {
        let result = load_custom_guidance(None);
        assert_eq!(result, TOOLING_CONTEXT);
    }

    #[test]
    fn test_load_custom_guidance_with_custom() {
        let custom = "Use poetry instead of uv for this project";
        let result = load_custom_guidance(Some(custom));
        assert!(result.contains(TOOLING_CONTEXT));
        assert!(result.contains("Custom Guidance"));
        assert!(result.contains(custom));
    }

    #[tokio::test]
    async fn test_load_system_prompt_override_returns_none_when_absent() {
        let result = load_system_prompt_override("__nonexistent_test_override__").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_system_prompt_override_returns_content_when_present() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file_path = dir.path().join("test_override.md");
        let mut f = std::fs::File::create(&file_path).expect("create file");
        writeln!(f, "Custom override content").expect("write file");
        drop(f);

        let content = tokio::fs::read_to_string(&file_path).await.ok();
        assert_eq!(content.as_deref(), Some("Custom override content\n"));
    }
}
