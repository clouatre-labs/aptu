//! GitHub OAuth device flow authentication.
//!
//! Implements the OAuth device flow for CLI authentication:
//! 1. Request device code from GitHub
//! 2. Display verification URL and user code to user
//! 3. Poll for access token after user authorizes
//! 4. Store token securely in system keychain

use anyhow::{Context, Result};
use keyring::Entry;
use octocrab::Octocrab;
use reqwest::header::ACCEPT;
use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, info, instrument};

use super::{KEYRING_SERVICE, KEYRING_USER};

/// OAuth scopes required for Aptu functionality.
const OAUTH_SCOPES: &[&str] = &["repo", "read:user"];

/// Creates a keyring entry for the GitHub token.
fn keyring_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER).context("Failed to create keyring entry")
}

/// Checks if a GitHub token is stored in the keyring.
#[instrument]
pub fn is_authenticated() -> bool {
    match keyring_entry() {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    }
}

/// Retrieves the stored GitHub token from the keyring.
///
/// Returns `None` if no token is stored or if keyring access fails.
#[allow(dead_code)] // Will be used by repos/issues/triage commands
#[instrument]
pub fn get_stored_token() -> Option<SecretString> {
    let entry = keyring_entry().ok()?;
    let password = entry.get_password().ok()?;
    debug!("Retrieved token from keyring");
    Some(SecretString::from(password))
}

/// Stores a GitHub token in the system keyring.
#[instrument(skip(token))]
pub fn store_token(token: &SecretString) -> Result<()> {
    let entry = keyring_entry()?;
    entry
        .set_password(token.expose_secret())
        .context("Failed to store token in keyring")?;
    info!("Token stored in system keyring");
    Ok(())
}

/// Deletes the stored GitHub token from the keyring.
#[instrument]
pub fn delete_token() -> Result<()> {
    let entry = keyring_entry()?;
    entry
        .delete_credential()
        .context("Failed to delete token from keyring")?;
    info!("Token deleted from keyring");
    Ok(())
}

/// Performs the GitHub OAuth device flow authentication.
///
/// This function:
/// 1. Requests a device code from GitHub
/// 2. Returns the verification URI and user code for display
/// 3. Polls GitHub until the user authorizes or times out
/// 4. Stores the resulting token in the system keychain
///
/// Requires `APTU_GH_CLIENT_ID` environment variable to be set.
#[instrument]
pub async fn authenticate(client_id: &SecretString) -> Result<()> {
    debug!("Starting OAuth device flow");

    // Build a client configured for GitHub's OAuth endpoints
    let crab = Octocrab::builder()
        .base_uri("https://github.com")
        .context("Failed to set base URI")?
        .add_header(ACCEPT, "application/json".to_string())
        .build()
        .context("Failed to build OAuth client")?;

    // Request device and user codes
    let codes = crab
        .authenticate_as_device(client_id, OAUTH_SCOPES)
        .await
        .context("Failed to request device code")?;

    // Display instructions to user
    println!();
    println!("To authenticate, visit:");
    println!();
    println!("    {}", codes.verification_uri);
    println!();
    println!("And enter the code:");
    println!();
    println!("    {}", codes.user_code);
    println!();
    println!("Waiting for authorization...");

    // Poll until user authorizes (octocrab handles backoff)
    let auth = codes
        .poll_until_available(&crab, client_id)
        .await
        .context("Authorization failed or timed out")?;

    // Store the access token
    let token = SecretString::from(auth.access_token.expose_secret().to_owned());
    store_token(&token)?;

    info!("Authentication successful");
    Ok(())
}

/// Creates an authenticated Octocrab client using the stored token.
///
/// Returns an error if no token is stored.
#[allow(dead_code)] // Will be used by repos/issues/triage commands
#[instrument]
pub fn create_client() -> Result<Octocrab> {
    let token = get_stored_token().context("Not authenticated - run `aptu auth` first")?;

    let client = Octocrab::builder()
        .personal_token(token.expose_secret().to_string())
        .build()
        .context("Failed to build GitHub client")?;

    debug!("Created authenticated GitHub client");
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_entry_creation() {
        // Just verify we can create an entry without panicking
        let result = keyring_entry();
        assert!(result.is_ok());
    }
}
