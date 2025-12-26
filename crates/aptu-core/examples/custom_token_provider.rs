// SPDX-License-Identifier: Apache-2.0

//! Implement a custom `TokenProvider` for credential resolution.
//!
//! Run with: `cargo run --example custom_token_provider -p aptu-core`

use aptu_core::TokenProvider;
use secrecy::SecretString;

/// In-memory token provider for demonstration.
struct MockProvider {
    github: Option<SecretString>,
}

impl TokenProvider for MockProvider {
    fn github_token(&self) -> Option<SecretString> {
        self.github.clone()
    }

    fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
        // Return a mock key for any provider
        Some(SecretString::from(format!("mock-{provider}-key")))
    }
}

fn main() {
    let provider = MockProvider {
        github: Some(SecretString::from("ghp_example")),
    };

    println!(
        "GitHub token present: {}",
        provider.github_token().is_some()
    );
    println!(
        "Gemini key present: {}",
        provider.ai_api_key("gemini").is_some()
    );
}
