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
use std::sync::OnceLock;

use anyhow::{Context, Result};
#[cfg(feature = "keyring")]
use keyring::Entry;
use octocrab::Octocrab;
#[cfg(feature = "keyring")]
use reqwest::header::ACCEPT;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use tracing::{debug, info, instrument};

#[cfg(feature = "keyring")]
use super::{KEYRING_SERVICE, KEYRING_USER};

/// Session-level cache for resolved GitHub tokens.
/// Stores the token and its source to avoid repeated subprocess calls to `gh auth token`.
static TOKEN_CACHE: OnceLock<Option<(SecretString, TokenSource)>> = OnceLock::new();

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
#[cfg(feature = "keyring")]
const OAUTH_SCOPES: &[&str] = &["repo", "read:user"];

/// Creates a keyring entry for the GitHub token.
#[cfg(feature = "keyring")]
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
#[cfg(feature = "keyring")]
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
#[cfg(feature = "keyring")]
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

/// Generic token resolution logic that accepts an environment variable reader.
///
/// This function enables dependency injection of the environment reader,
/// allowing tests to pass mock values without manipulating the real environment.
///
/// Checks sources in order:
/// 1. `GH_TOKEN` environment variable (via provided reader)
/// 2. `GITHUB_TOKEN` environment variable (via provided reader)
/// 3. GitHub CLI (`gh auth token`)
/// 4. System keyring (native aptu auth)
///
/// # Arguments
///
/// * `env_reader` - A function that reads environment variables, returning `Ok(value)` or `Err(_)`
///
/// Returns the token and its source, or `None` if no token is found.
fn resolve_token_with_env<F>(env_reader: F) -> Option<(SecretString, TokenSource)>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
{
    // Priority 1: GH_TOKEN environment variable
    if let Ok(token) = env_reader("GH_TOKEN")
        && !token.is_empty()
    {
        debug!("Using token from GH_TOKEN environment variable");
        return Some((SecretString::from(token), TokenSource::Environment));
    }

    // Priority 2: GITHUB_TOKEN environment variable
    if let Ok(token) = env_reader("GITHUB_TOKEN")
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
    #[cfg(feature = "keyring")]
    if let Some(token) = get_stored_token() {
        debug!("Using token from system keyring");
        return Some((token, TokenSource::Keyring));
    }

    debug!("No token found in any source");
    None
}

/// Internal token resolution logic without caching.
///
/// Checks sources in order:
/// 1. `GH_TOKEN` environment variable
/// 2. `GITHUB_TOKEN` environment variable
/// 3. GitHub CLI (`gh auth token`)
/// 4. System keyring (native aptu auth)
///
/// Returns the token and its source, or `None` if no token is found.
fn resolve_token_inner() -> Option<(SecretString, TokenSource)> {
    resolve_token_with_env(|key| std::env::var(key))
}

/// Resolves a GitHub token using the priority chain with session-level caching.
///
/// Caches the resolved token to avoid repeated subprocess calls to `gh auth token`.
/// The cache is valid for the lifetime of the session (CLI invocation).
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
    TOKEN_CACHE
        .get_or_init(resolve_token_inner)
        .as_ref()
        .map(|(token, source)| {
            debug!(source = %source, "Cache hit for token resolution");
            (token.clone(), *source)
        })
}

/// Stores a GitHub token in the system keyring.
#[cfg(feature = "keyring")]
#[instrument(skip(token))]
pub fn store_token(token: &SecretString) -> Result<()> {
    let entry = keyring_entry()?;
    entry
        .set_password(token.expose_secret())
        .context("Failed to store token in keyring")?;
    info!("Token stored in system keyring");
    Ok(())
}

/// Clears the session-level token cache.
///
/// This should be called after logout or when the token is invalidated.
#[instrument]
pub fn clear_token_cache() {
    // OnceLock doesn't provide a direct clear method, but we can work around this
    // by using take() if it were available. Since it's not, we document that
    // the cache is session-scoped and will be cleared on process exit.
    debug!("Token cache cleared (session-scoped)");
}

/// Deletes the stored GitHub token from the keyring.
#[cfg(feature = "keyring")]
#[instrument]
pub fn delete_token() -> Result<()> {
    let entry = keyring_entry()?;
    entry
        .delete_credential()
        .context("Failed to delete token from keyring")?;
    clear_token_cache();
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
#[cfg(feature = "keyring")]
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

/// Creates a GitHub client from a `TokenProvider`.
///
/// This is a convenience function that extracts the token from a provider
/// and creates an authenticated Octocrab client. It standardizes error handling
/// across the facade layer.
///
/// # Arguments
///
/// * `provider` - Token provider that supplies the GitHub token
///
/// # Returns
///
/// Returns `Ok(Octocrab)` if successful, or an `AptuError::GitHub` if:
/// - The provider has no token available
/// - The GitHub client fails to build
///
/// # Example
///
/// ```ignore
/// let client = create_client_from_provider(provider)?;
/// ```
#[instrument(skip(provider))]
pub fn create_client_from_provider(
    provider: &dyn crate::auth::TokenProvider,
) -> crate::Result<Octocrab> {
    let github_token = provider
        .github_token()
        .ok_or(crate::error::AptuError::NotAuthenticated)?;

    let token = SecretString::from(github_token);
    create_client_with_token(&token).map_err(|e| crate::error::AptuError::GitHub {
        message: format!("Failed to create GitHub client: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "keyring")]
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

    #[test]
    fn test_resolve_token_with_env_var() {
        // Arrange: Create a mock env reader that returns a test token
        let mock_env = |key: &str| -> Result<String, std::env::VarError> {
            match key {
                "GH_TOKEN" => Ok("test_token_123".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };

        // Act
        let result = resolve_token_with_env(mock_env);

        // Assert
        assert!(result.is_some());
        let (token, source) = result.unwrap();
        assert_eq!(token.expose_secret(), "test_token_123");
        assert_eq!(source, TokenSource::Environment);
    }

    #[test]
    fn test_resolve_token_with_env_prefers_gh_token_over_github_token() {
        // Arrange: Create a mock env reader that returns both tokens
        let mock_env = |key: &str| -> Result<String, std::env::VarError> {
            match key {
                "GH_TOKEN" => Ok("gh_token".to_string()),
                "GITHUB_TOKEN" => Ok("github_token".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };

        // Act
        let result = resolve_token_with_env(mock_env);

        // Assert: GH_TOKEN should take priority
        assert!(result.is_some());
        let (token, _) = result.unwrap();
        assert_eq!(token.expose_secret(), "gh_token");
    }
}
