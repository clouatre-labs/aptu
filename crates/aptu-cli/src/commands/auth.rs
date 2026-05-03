// SPDX-License-Identifier: Apache-2.0

//! GitHub OAuth authentication command.

use anyhow::Result;
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
pub async fn run_status() -> Result<crate::commands::types::AuthStatusResult> {
    match auth::resolve_token() {
        Some((token, source)) => {
            let username = match auth::create_client_with_token(&token) {
                Ok(client) => match client.current().user().await {
                    Ok(user) => Some(user.login),
                    Err(_) => None,
                },
                Err(_) => None,
            };

            Ok(crate::commands::types::AuthStatusResult {
                authenticated: true,
                method: Some(source),
                username,
            })
        }
        None => Ok(crate::commands::types::AuthStatusResult {
            authenticated: false,
            method: None,
            username: None,
        }),
    }
}
