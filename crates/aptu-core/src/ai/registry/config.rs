// SPDX-License-Identifier: Apache-2.0

//! Static provider configuration registry.

use super::consts::{PROVIDER_ANTHROPIC, PROVIDER_GEMINI, PROVIDER_OPENROUTER, PROVIDER_ZAI};

/// Configuration for an AI provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProviderConfig {
    /// Provider identifier (lowercase, used in config files)
    pub name: &'static str,

    /// Human-readable provider name for UI display
    pub display_name: &'static str,

    /// API base URL for this provider
    pub api_url: &'static str,

    /// Environment variable name for API key
    pub api_key_env: &'static str,

    /// Default model name for this provider
    pub model: &'static str,

    /// Default maximum tokens for API responses
    pub max_tokens: u32,

    /// Default temperature for API requests
    pub temperature: f32,
}

/// Static registry of all supported AI providers
pub static PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig {
        name: PROVIDER_GEMINI,
        display_name: "Google Gemini",
        api_url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        api_key_env: "GEMINI_API_KEY",
        model: "gemini-3.5-flash-lite",
        max_tokens: 4096,
        temperature: 0.3,
    },
    ProviderConfig {
        name: PROVIDER_OPENROUTER,
        display_name: "OpenRouter",
        api_url: "https://openrouter.ai/api/v1/chat/completions",
        api_key_env: "OPENROUTER_API_KEY",
        model: "mistralai/mistral-small-2603",
        max_tokens: 4096,
        temperature: 0.3,
    },
    ProviderConfig {
        name: PROVIDER_ZAI,
        display_name: "Z.AI (Zhipu)",
        api_url: "https://api.z.ai/api/paas/v4/chat/completions",
        api_key_env: "ZAI_API_KEY",
        model: "glm-5.3",
        max_tokens: 4096,
        temperature: 0.3,
    },
    ProviderConfig {
        name: PROVIDER_ANTHROPIC,
        display_name: "Anthropic",
        api_url: "https://api.anthropic.com/v1/chat/completions",
        api_key_env: "ANTHROPIC_API_KEY",
        model: "claude-sonnet-5",
        max_tokens: 4096,
        temperature: 0.3,
    },
];

/// Retrieves a provider configuration by name.
///
/// # Arguments
///
/// * `name` - The provider name (case-sensitive, lowercase)
///
/// # Returns
///
/// Some(ProviderConfig) if found, None otherwise.
///
/// # Examples
///
/// ```
/// use aptu_core::ai::registry::get_provider;
///
/// let provider = get_provider("openrouter");
/// assert!(provider.is_some());
/// assert_eq!(provider.unwrap().display_name, "OpenRouter");
/// ```
#[must_use]
pub fn get_provider(name: &str) -> Option<&'static ProviderConfig> {
    PROVIDERS.iter().find(|p| p.name == name)
}

/// Returns all available providers.
///
/// # Returns
///
/// A slice of all `ProviderConfig` entries in the registry.
///
/// # Examples
///
/// ```
/// use aptu_core::ai::registry::all_providers;
///
/// let providers = all_providers();
/// assert_eq!(providers.len(), 4);
/// ```
#[must_use]
pub fn all_providers() -> &'static [ProviderConfig] {
    PROVIDERS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_provider_gemini() {
        let provider = get_provider("gemini");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Google Gemini");
        assert_eq!(provider.api_key_env, "GEMINI_API_KEY");
    }

    #[test]
    fn test_get_provider_openrouter() {
        let provider = get_provider("openrouter");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "OpenRouter");
        assert_eq!(provider.api_key_env, "OPENROUTER_API_KEY");
    }

    #[test]
    fn test_get_provider_not_found() {
        for removed in ["groq", "cerebras", "zenmux"] {
            assert!(
                get_provider(removed).is_none(),
                "{removed} should be removed"
            );
        }
        let provider = get_provider("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_get_provider_case_sensitive() {
        let provider = get_provider("OpenRouter");
        assert!(
            provider.is_none(),
            "Provider lookup should be case-sensitive"
        );
    }

    #[test]
    fn test_all_providers_count() {
        let providers = all_providers();
        assert_eq!(providers.len(), 4, "Should have exactly 4 providers");
    }

    #[test]
    fn test_all_providers_have_unique_names() {
        let providers = all_providers();
        let mut names = Vec::new();
        for provider in providers {
            assert!(
                !names.contains(&provider.name),
                "Duplicate provider name: {}",
                provider.name
            );
            names.push(provider.name);
        }
    }

    #[test]
    fn test_get_provider_zai() {
        let provider = get_provider("zai");
        assert!(provider.is_some());
        let provider = provider.unwrap();
        assert_eq!(provider.display_name, "Z.AI (Zhipu)");
        assert_eq!(provider.api_key_env, "ZAI_API_KEY");
    }

    #[test]
    fn test_provider_api_urls_valid() {
        let providers = all_providers();
        for provider in providers {
            assert!(
                provider.api_url.starts_with("https://"),
                "Provider {} API URL should use HTTPS",
                provider.name
            );
        }
    }

    #[test]
    fn test_provider_api_key_env_not_empty() {
        let providers = all_providers();
        for provider in providers {
            assert!(
                !provider.api_key_env.is_empty(),
                "Provider {} should have API key env var",
                provider.name
            );
        }
    }
}
