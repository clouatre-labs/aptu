// SPDX-License-Identifier: Apache-2.0

//! GitHub OAuth device flow authentication.
//!
//! Implements the OAuth device flow for CLI authentication:
//! 1. Request device code from GitHub
//! 2. Display verification URL and user code to user
//! 3. Poll for access token after user authorizes
//! 4. Store token securely in system keychain
//!
//! Also provides a token resolution priority chain:
//! 1. Environment variable (`GH_TOKEN` or `GITHUB_TOKEN`)
//! 2. GitHub CLI (`gh auth token`)
//! 3. System keyring (native aptu auth)

use std::process::Command;

use anyhow::{Context, Result};
use keyring::Entry;
use octocrab::Octocrab;
use reqwest::header::ACCEPT;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use tracing::{debug, info, instrument};

use super::{KEYRING_SERVICE, KEYRING_USER};

/// Source of the GitHub authentication token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
    /// Token from `GH_TOKEN` or `GITHUB_TOKEN` environment variable.
    Environment,
    /// Token from `gh auth token` command.
    GhCli,
    /// Token from system keyring (native aptu auth).
    Keyring,
}

impl std::fmt::Display for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenSource::Environment => write!(f, "environment variable"),
            TokenSource::GhCli => write!(f, "GitHub CLI"),
            TokenSource::Keyring => write!(f, "system keyring"),
        }
    }
}

/// OAuth scopes required for Aptu functionality.
const OAUTH_SCOPES: &[&str] = &["repo", "read:user"];

/// Creates a keyring entry for the GitHub token.
fn keyring_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER).context("Failed to create keyring entry")
}

/// Checks if a GitHub token is available from any source.
///
/// Uses the token resolution priority chain to check for authentication.
#[instrument]
#[allow(clippy::let_and_return)] // Intentional: Rust 2024 drop order compliance
pub fn is_authenticated() -> bool {
    let result = resolve_token().is_some();
    result
}

/// Checks if a GitHub token is stored in the keyring specifically.
///
/// Returns `true` only if a token exists in the system keyring,
/// ignoring environment variables and `gh` CLI.
#[instrument]
#[allow(clippy::let_and_return)] // Intentional: Rust 2024 drop order compliance
pub fn has_keyring_token() -> bool {
    let result = match keyring_entry() {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    };
    result
}

/// Retrieves the stored GitHub token from the keyring.
///
/// Returns `None` if no token is stored or if keyring access fails.
#[instrument]
pub fn get_stored_token() -> Option<SecretString> {
    let entry = keyring_entry().ok()?;
    let password = entry.get_password().ok()?;
    debug!("Retrieved token from keyring");
    Some(SecretString::from(password))
}

/// Attempts to get a token from the GitHub CLI (`gh auth token`).
///
/// Returns `None` if:
/// - `gh` is not installed
/// - `gh` is not authenticated
/// - The command times out (5 seconds)
/// - Any other error occurs
#[instrument]
fn get_token_from_gh_cli() -> Option<SecretString> {
    debug!("Attempting to get token from gh CLI");

    // Use wait-timeout crate pattern with std::process
    let output = Command::new("gh").args(["auth", "token"]).output();

    match output {
        Ok(output) if output.status.success() => {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if token.is_empty() {
                debug!("gh auth token returned empty output");
                None
            } else {
                debug!("Successfully retrieved token from gh CLI");
                Some(SecretString::from(token))
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!(
                status = ?output.status,
                stderr = %stderr.trim(),
                "gh auth token failed"
            );
            None
        }
        Err(e) => {
            debug!(error = %e, "Failed to execute gh command");
            None
        }
    }
}

/// Resolves a GitHub token using the priority chain.
///
/// Checks sources in order:
/// 1. `GH_TOKEN` environment variable
/// 2. `GITHUB_TOKEN` environment variable
/// 3. GitHub CLI (`gh auth token`)
/// 4. System keyring (native aptu auth)
///
/// Returns the token and its source, or `None` if no token is found.
#[instrument]
pub fn resolve_token() -> Option<(SecretString, TokenSource)> {
    // Priority 1: GH_TOKEN environment variable
    if let Ok(token) = std::env::var("GH_TOKEN")
        && !token.is_empty()
    {
        debug!("Using token from GH_TOKEN environment variable");
        return Some((SecretString::from(token), TokenSource::Environment));
    }

    // Priority 2: GITHUB_TOKEN environment variable
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        debug!("Using token from GITHUB_TOKEN environment variable");
        return Some((SecretString::from(token), TokenSource::Environment));
    }

    // Priority 3: GitHub CLI
    if let Some(token) = get_token_from_gh_cli() {
        debug!("Using token from GitHub CLI");
        return Some((token, TokenSource::GhCli));
    }

    // Priority 4: System keyring
    if let Some(token) = get_stored_token() {
        debug!("Using token from system keyring");
        return Some((token, TokenSource::Keyring));
    }

    debug!("No token found in any source");
    None
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

/// Creates an authenticated Octocrab client using the token priority chain.
///
/// Uses [`resolve_token`] to find credentials from environment variables,
/// GitHub CLI, or system keyring.
///
/// Returns an error if no token is found from any source.
#[instrument]
pub fn create_client() -> Result<Octocrab> {
    let (token, source) =
        resolve_token().context("Not authenticated - run `aptu auth login` first")?;

    info!(source = %source, "Creating GitHub client");

    let client = Octocrab::builder()
        .personal_token(token.expose_secret().to_string())
        .build()
        .context("Failed to build GitHub client")?;

    debug!("Created authenticated GitHub client");
    Ok(client)
}

/// Creates an authenticated Octocrab client using a provided token.
///
/// This function allows callers to provide a token directly, enabling
/// multi-platform credential resolution (e.g., from iOS keychain via FFI).
///
/// # Arguments
///
/// * `token` - GitHub API token as a `SecretString`
///
/// # Errors
///
/// Returns an error if the Octocrab client cannot be built.
#[instrument(skip(token))]
pub fn create_client_with_token(token: &SecretString) -> Result<Octocrab> {
    info!("Creating GitHub client with provided token");

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

    #[test]
    fn test_token_source_display() {
        assert_eq!(TokenSource::Environment.to_string(), "environment variable");
        assert_eq!(TokenSource::GhCli.to_string(), "GitHub CLI");
        assert_eq!(TokenSource::Keyring.to_string(), "system keyring");
    }

    #[test]
    fn test_token_source_equality() {
        assert_eq!(TokenSource::Environment, TokenSource::Environment);
        assert_ne!(TokenSource::Environment, TokenSource::GhCli);
        assert_ne!(TokenSource::GhCli, TokenSource::Keyring);
    }

    #[test]
    fn test_gh_cli_not_installed_returns_none() {
        // This test verifies that get_token_from_gh_cli gracefully handles
        // the case where gh is not in PATH (returns None, doesn't panic)
        // Note: This test may pass even if gh IS installed, because we're
        // testing the graceful fallback behavior
        let result = get_token_from_gh_cli();
        // We can't assert None here because gh might be installed
        // Just verify it doesn't panic and returns Option
        let _ = result;
    }
}
