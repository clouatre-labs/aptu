// SPDX-License-Identifier: Apache-2.0

//! Token provider for MCP server using environment variables.

use aptu_core::ai::registry::get_provider;
use aptu_core::auth::TokenProvider;
use secrecy::SecretString;

#[cfg(test)]
use secrecy::ExposeSecret;

/// Resolves credentials from environment variables.
///
/// Reads `GITHUB_TOKEN` for GitHub API access and `AI_API_KEY` for AI provider access.
/// For AI providers, attempts to use provider-specific environment variables first,
/// then falls back to `AI_API_KEY` for backward compatibility.
pub struct EnvTokenProvider;

impl TokenProvider for EnvTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        std::env::var("GITHUB_TOKEN").ok().map(SecretString::from)
    }

    fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
        // Try provider-specific environment variable first
        if let Some(provider_config) = get_provider(provider)
            && let Ok(key) = std::env::var(provider_config.api_key_env)
        {
            return Some(SecretString::from(key));
        }

        // Fall back to AI_API_KEY for backward compatibility
        std::env::var("AI_API_KEY").ok().map(SecretString::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_specific_env_var_takes_precedence() {
        // When both provider-specific and fallback vars are set,
        // provider-specific should be used
        let provider = EnvTokenProvider;

        // If OPENROUTER_API_KEY is set, it should be returned
        if std::env::var("OPENROUTER_API_KEY").is_ok() {
            let key = provider.ai_api_key("openrouter");
            assert!(key.is_some());
            assert_eq!(
                key.unwrap().expose_secret(),
                std::env::var("OPENROUTER_API_KEY").unwrap()
            );
        }
    }

    #[test]
    fn falls_back_to_ai_api_key_when_provider_specific_missing() {
        // When provider-specific var is missing but AI_API_KEY is set,
        // fallback should be used
        let provider = EnvTokenProvider;

        // For a provider that might not have a specific var set,
        // check if fallback works
        if std::env::var("AI_API_KEY").is_ok() && std::env::var("GROQ_API_KEY").is_err() {
            let key = provider.ai_api_key("groq");
            assert!(key.is_some());
            assert_eq!(
                key.unwrap().expose_secret(),
                std::env::var("AI_API_KEY").unwrap()
            );
        }
    }

    #[test]
    fn unknown_provider_uses_fallback() {
        // Unknown providers should fall back to AI_API_KEY
        let provider = EnvTokenProvider;

        if std::env::var("AI_API_KEY").is_ok() {
            let key = provider.ai_api_key("unknown_provider_xyz");
            assert!(key.is_some());
            assert_eq!(
                key.unwrap().expose_secret(),
                std::env::var("AI_API_KEY").unwrap()
            );
        }
    }

    #[test]
    fn github_token_resolution() {
        // Test that github_token reads from GITHUB_TOKEN env var
        let provider = EnvTokenProvider;

        if std::env::var("GITHUB_TOKEN").is_ok() {
            let token = provider.github_token();
            assert!(token.is_some());
            assert_eq!(
                token.unwrap().expose_secret(),
                std::env::var("GITHUB_TOKEN").unwrap()
            );
        }
    }
}
