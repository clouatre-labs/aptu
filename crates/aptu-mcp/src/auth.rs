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

/// Resolves credentials from HTTP request headers.
///
/// Extracts per-request credentials injected by the rmcp [`StreamableHttpService`].
/// Header names are derived from provider API key environment variable names:
/// `GEMINI_API_KEY` -> `x-gemini-api-key` (lowercase, underscores to hyphens, prepend `x-`).
/// Falls back to `EnvTokenProvider` if a header is absent.
///
/// [`StreamableHttpService`]: rmcp::transport::StreamableHttpService
pub struct HeaderTokenProvider {
    headers: std::collections::HashMap<String, String>,
}

impl HeaderTokenProvider {
    /// Constructs a new `HeaderTokenProvider` from HTTP request headers.
    ///
    /// Copies all header names (lowercased) and values from the request into an owned map.
    /// # Arguments
    /// * `parts` - HTTP request parts containing headers
    pub fn new(parts: &http::request::Parts) -> Self {
        let mut headers = std::collections::HashMap::new();
        for (name, value) in &parts.headers {
            if let Ok(val_str) = value.to_str() {
                headers.insert(name.to_string().to_lowercase(), val_str.to_string());
            }
        }
        Self { headers }
    }

    /// Derives an HTTP header name from an AI provider's API key environment variable.
    ///
    /// Transformation: `GEMINI_API_KEY` -> `x-gemini-api-key`.
    /// Returns `None` if the provider is unknown (no `api_key_env` defined).
    fn derive_header_name(provider: &str) -> Option<String> {
        get_provider(provider)
            .map(|p| format!("x-{}", p.api_key_env.to_lowercase().replace('_', "-")))
    }
}

impl TokenProvider for HeaderTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        self.headers
            .get("x-github-token")
            .map(|v| SecretString::from(v.clone()))
            .or_else(|| EnvTokenProvider.github_token())
    }

    fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
        let header_name = Self::derive_header_name(provider)?;
        self.headers
            .get(&header_name)
            .map(|v| SecretString::from(v.clone()))
            .or_else(|| EnvTokenProvider.ai_api_key(provider))
    }
}

/// Creates a token provider from optional HTTP request parts.
///
/// If parts are provided, returns `HeaderTokenProvider` for per-request credential forwarding.
/// Otherwise, returns `EnvTokenProvider` for environment-based credentials (stdio transport).
pub fn make_provider(
    parts_opt: Option<&http::request::Parts>,
) -> Box<dyn TokenProvider + Send + Sync> {
    match parts_opt {
        Some(parts) => Box::new(HeaderTokenProvider::new(parts)),
        None => Box::new(EnvTokenProvider),
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;

    #[test]
    fn test_header_token_provider_github_token_present() {
        // Arrange: create mock HTTP parts with x-github-token header
        let mut headers = http::HeaderMap::new();
        headers.insert("x-github-token", "ghp_test123".parse().unwrap());

        // Create request and extract parts
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/test")
            .header("x-github-token", "ghp_test123")
            .body(())
            .unwrap();
        let (parts, _) = req.into_parts();

        // Act
        let provider = HeaderTokenProvider::new(&parts);
        let token = provider.github_token();

        // Assert
        assert!(token.is_some());
        assert_eq!(token.unwrap().expose_secret(), "ghp_test123");
    }

    #[test]
    fn test_header_token_provider_github_token_fallback() {
        // Arrange: create mock HTTP parts without x-github-token
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/test")
            .body(())
            .unwrap();
        let (parts, _) = req.into_parts();

        // Act: provider should attempt fallback to EnvTokenProvider
        let provider = HeaderTokenProvider::new(&parts);
        let token = provider.github_token();

        // Assert: token presence depends on environment, so we just check it doesn't panic
        let _ = token;
    }

    #[test]
    fn test_header_name_derivation() {
        // Arrange: test the header name derivation for a known provider
        // GEMINI_API_KEY -> x-gemini-api-key
        let header_name = HeaderTokenProvider::derive_header_name("gemini");

        // Assert
        assert_eq!(header_name, Some("x-gemini-api-key".to_string()));
    }

    #[test]
    fn test_header_name_derivation_openrouter() {
        // Test another provider: OPENROUTER_API_KEY -> x-openrouter-api-key
        let header_name = HeaderTokenProvider::derive_header_name("openrouter");
        assert_eq!(header_name, Some("x-openrouter-api-key".to_string()));
    }

    #[test]
    fn test_make_provider_with_parts() {
        // Arrange
        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri("/test")
            .header("x-github-token", "ghp_test456")
            .body(())
            .unwrap();
        let (parts, _) = req.into_parts();

        // Act
        let provider = make_provider(Some(&parts));
        let token = provider.github_token();

        // Assert
        assert!(token.is_some());
        assert_eq!(token.unwrap().expose_secret(), "ghp_test456");
    }

    #[test]
    fn test_make_provider_without_parts() {
        // Act: when parts_opt is None, should use EnvTokenProvider
        let provider = make_provider(None);
        let token = provider.github_token();

        // Assert: just check it doesn't panic
        let _ = token;
    }
}
