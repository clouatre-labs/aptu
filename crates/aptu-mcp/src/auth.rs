// SPDX-License-Identifier: Apache-2.0

//! Token provider for MCP server using environment variables.

use aptu_core::auth::TokenProvider;
use secrecy::SecretString;

/// Resolves credentials from environment variables.
///
/// Reads `GITHUB_TOKEN` for GitHub API access and `AI_API_KEY` for AI provider access.
pub struct EnvTokenProvider;

impl TokenProvider for EnvTokenProvider {
    fn github_token(&self) -> Option<SecretString> {
        std::env::var("GITHUB_TOKEN").ok().map(SecretString::from)
    }

    fn ai_api_key(&self, _provider: &str) -> Option<SecretString> {
        std::env::var("AI_API_KEY").ok().map(SecretString::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn returns_none_when_env_vars_unset() {
        // Clear env vars for test isolation
        // SAFETY: Test runs single-threaded; no other threads access these vars.
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("AI_API_KEY");
        }

        let provider = EnvTokenProvider;
        assert!(provider.github_token().is_none());
        assert!(provider.ai_api_key("openrouter").is_none());
    }
}
