// SPDX-License-Identifier: Apache-2.0

//! GitHub integration module.
//!
//! Provides authentication and API client functionality for GitHub.

pub mod auth;
pub mod graphql;
pub mod issues;
pub mod pulls;
pub mod ratelimit;

/// OAuth Client ID for Aptu CLI (safe to embed per RFC 8252).
///
/// This is a public client ID for native/CLI applications. Per OAuth 2.0 for
/// Native Apps (RFC 8252), client credentials in native apps cannot be kept
/// confidential and are safe to embed in source code.
pub const OAUTH_CLIENT_ID: &str = "Ov23lifiYQrh6Ga7Hpyr";

/// Keyring service name for storing credentials.
pub const KEYRING_SERVICE: &str = "aptu";

/// Keyring username for the GitHub token.
pub const KEYRING_USER: &str = "github_token";
