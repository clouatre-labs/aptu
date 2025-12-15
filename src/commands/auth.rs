//! GitHub OAuth authentication command.

use anyhow::Result;
use console::style;
use secrecy::SecretString;
use tracing::info;

use crate::github::{auth, OAUTH_CLIENT_ID};

/// Run the authentication command.
///
/// If `logout` is true, removes stored credentials.
/// Otherwise, initiates the OAuth device flow.
pub async fn run(logout: bool) -> Result<()> {
    if logout {
        return run_logout();
    }

    // Check if already authenticated via any source
    if let Some((_, source)) = auth::resolve_token() {
        println!(
            "{} Already authenticated with GitHub (via {}).",
            style("!").yellow().bold(),
            source
        );
        println!(
            "Run {} to remove keyring token and re-authenticate.",
            style("aptu auth --logout").cyan()
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

/// Remove stored credentials.
fn run_logout() -> Result<()> {
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
