// SPDX-License-Identifier: Apache-2.0

//! GitHub OAuth authentication command.

use anyhow::Result;
use aptu_core::github::{OAUTH_CLIENT_ID, auth};
use console::style;
use secrecy::SecretString;
use tracing::info;

/// Run the login command - authenticate with GitHub.
pub async fn run_login() -> Result<()> {
    // Check if already authenticated via any source
    if let Some((_, source)) = auth::resolve_token() {
        println!(
            "{} Already authenticated with GitHub (via {}).",
            style("!").yellow().bold(),
            source
        );
        println!(
            "Run {} to remove keyring token and re-authenticate.",
            style("aptu auth logout").cyan()
        );
        return Ok(());
    }

    let client_id = SecretString::from(OAUTH_CLIENT_ID);

    println!(
        "{} Starting GitHub authentication...",
        style("*").cyan().bold()
    );

    auth::authenticate(&client_id).await?;

    println!();
    println!(
        "{} Successfully authenticated with GitHub!",
        style("*").green().bold()
    );

    Ok(())
}

/// Run the logout command - remove stored credentials.
pub fn run_logout() -> Result<()> {
    if !auth::has_keyring_token() {
        println!("{} No token stored in keyring.", style("!").yellow().bold());
        return Ok(());
    }

    auth::delete_token()?;

    info!("Logged out from GitHub");
    println!(
        "{} Logged out from GitHub. Token removed from keychain.",
        style("*").green().bold()
    );

    Ok(())
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
