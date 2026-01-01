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
pub const TOOLING_CONTEXT: &str = r"## Best Practices Context (2026)

When providing recommendations, use these current best practices:

### Python
- **Linting & Formatting**: Use `ruff` (replaces flake8, isort, black)
- **Type Checking**: Use `pyright` (replaces mypy for better performance and accuracy)
- **Security Scanning**: Use `ruff` with security rules (replaces bandit)
- **Package Management**: Use `uv` (fast, modern replacement for pip/poetry)
- **Testing**: pytest with pytest-cov for coverage

### JavaScript/TypeScript
- **Linting & Formatting**: Use `biome` (replaces eslint+prettier with better performance)
- **Type Checking**: Use TypeScript with strict mode
- **Package Management**: Use `bun` (fastest, all-in-one toolkit) or `pnpm` (fast, efficient). Avoid npm/yarn.
- **Testing**: Use `vitest` (Vite-native, replaces jest) or `bun test` (if using Bun runtime)

### Rust
- **Edition**: Check project's Cargo.toml for edition (2021 or 2024), use appropriate idioms
- **Formatting**: Use `rustfmt` (built-in)
- **Linting**: Use `clippy` (built-in)
- **Testing**: Use built-in test framework with `cargo test`
- **Code Style**: Prefer iterators over loops, use `?` for error propagation

### AI Models (2026)
- **Claude**: Sonnet/Opus 4.5 (NOT 3.x)
- **OpenAI**: GPT-5.2/5.1 (NOT GPT-4)
- **Google**: Gemini 3 Flash/Pro (NOT 2.x/1.x)
- Consider cost, latency, and capability trade-offs";

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
}
