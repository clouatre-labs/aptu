//! GitHub OAuth authentication command.

use anyhow::{Context, Result};
use console::style;
use secrecy::SecretString;
use tracing::info;

use crate::github::auth;

/// Run the authentication command.
///
/// If `logout` is true, removes stored credentials.
/// Otherwise, initiates the OAuth device flow.
pub async fn run(logout: bool) -> Result<()> {
    if logout {
        return run_logout();
    }

    // Check if already authenticated
    if auth::is_authenticated() {
        println!(
            "{} Already authenticated with GitHub.",
            style("!").yellow().bold()
        );
        println!(
            "Run {} to re-authenticate.",
            style("aptu auth --logout").cyan()
        );
        return Ok(());
    }

    // Get client ID from environment
    let client_id = std::env::var("APTU_GH_CLIENT_ID").context(
        "APTU_GH_CLIENT_ID environment variable not set.\n\n\
         To use Aptu, you need to create a GitHub OAuth App:\n\
         1. Go to https://github.com/settings/developers\n\
         2. Click 'New OAuth App'\n\
         3. Set Application name: Aptu\n\
         4. Set Homepage URL: https://github.com/clouatre-labs/aptu\n\
         5. Set Authorization callback URL: http://localhost (not used)\n\
         6. Copy the Client ID and set: export APTU_GH_CLIENT_ID=<your-client-id>",
    )?;

    let client_id = SecretString::from(client_id);

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
    if !auth::is_authenticated() {
        println!(
            "{} Not currently authenticated.",
            style("!").yellow().bold()
        );
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
