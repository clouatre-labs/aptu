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
pub fn run_status() {
    match auth::resolve_token() {
        Some((_, source)) => {
            println!(
                "{} Authenticated with GitHub (via {}).",
                style("*").green().bold(),
                source
            );
        }
        None => {
            println!(
                "{} Not authenticated. Run {} to authenticate.",
                style("!").yellow().bold(),
                style("aptu auth login").cyan()
            );
        }
    }
}
