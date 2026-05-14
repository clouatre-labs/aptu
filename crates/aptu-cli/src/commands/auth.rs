// SPDX-License-Identifier: Apache-2.0

//! GitHub OAuth authentication command.

use anyhow::Result;
use aptu_core::AppConfig;
use aptu_core::github::{OAUTH_CLIENT_ID, auth};
use secrecy::SecretString;
use tracing::info;

use crate::commands::types::AuthActionResult;

/// Run the login command - authenticate with GitHub.
pub async fn run_login() -> Result<AuthActionResult> {
    // Check if already authenticated via any source
    if let Some((_, source)) = auth::resolve_token() {
        return Ok(AuthActionResult {
            action: "login".to_string(),
            message: format!(
                "Already authenticated with GitHub (via {source}). Run `aptu auth logout` to remove keyring token and re-authenticate."
            ),
        });
    }

    // Check for Claude credentials file
    let has_claude_creds = if let Some(home) = dirs::home_dir() {
        home.join(".claude").join("credentials.json").exists()
    } else {
        false
    };

    if has_claude_creds {
        println!("Found Claude credentials at ~/.claude/credentials.json");
        println!("You can use your existing Claude subscription for AI features.");
    }

    let client_id = SecretString::from(OAUTH_CLIENT_ID);

    auth::authenticate(&client_id).await?;

    Ok(AuthActionResult {
        action: "login".to_string(),
        message: "Successfully authenticated with GitHub!".to_string(),
    })
}

/// Run the logout command - remove stored credentials.
pub fn run_logout() -> Result<AuthActionResult> {
    if !auth::has_keyring_token() {
        return Ok(AuthActionResult {
            action: "logout".to_string(),
            message: "No token stored in keyring.".to_string(),
        });
    }

    auth::delete_token()?;

    info!("Logged out from GitHub");
    Ok(AuthActionResult {
        action: "logout".to_string(),
        message: "Logged out from GitHub. Token removed from keychain.".to_string(),
    })
}

/// Run the status command - show current authentication state.
pub async fn run_status(config: &AppConfig) -> Result<crate::commands::types::AuthStatusResult> {
    // Get GitHub auth status
    let (authenticated, method, username) = match auth::resolve_token() {
        Some((token, source)) => {
            let username = match auth::create_client_with_token(&token) {
                Ok(client) => match client.current().user().await {
                    Ok(user) => Some(user.login),
                    Err(_) => None,
                },
                Err(_) => None,
            };
            (true, Some(source), username)
        }
        None => (false, None, None),
    };

    // Get AI provider auth status
    let (ai_provider, ai_auth_method) = {
        let provider_name = &config.ai.provider;
        let auth_method = match aptu_core::ai::AiClient::from_keyring_oauth(&config.ai) {
            Ok(Some(_client)) => Some((provider_name.clone(), "oauth".to_string())),
            Ok(None) => {
                match aptu_core::ai::AiClient::from_claude_credentials(&config.ai) {
                    Ok(Some(_client)) => Some((provider_name.clone(), "oauth".to_string())),
                    Ok(None) => {
                        // Check if API key is available
                        if std::env::var(format!("{}_API_KEY", provider_name.to_uppercase()))
                            .is_ok()
                        {
                            Some((provider_name.clone(), "api-key".to_string()))
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        };
        match auth_method {
            Some((provider, method)) => (Some(provider), Some(method)),
            None => (None, None),
        }
    };

    Ok(crate::commands::types::AuthStatusResult {
        authenticated,
        method,
        username,
        ai_provider,
        ai_auth_method,
    })
}
