//! GitHub integration module.
//!
//! Provides authentication and API client functionality for GitHub.

pub mod auth;

/// Keyring service name for storing credentials.
pub const KEYRING_SERVICE: &str = "aptu";

/// Keyring username for the GitHub token.
pub const KEYRING_USER: &str = "github_token";
