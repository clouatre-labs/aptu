//! GitHub OAuth authentication command.

use anyhow::Result;

/// Authenticate with GitHub via OAuth device flow.
pub async fn run() -> Result<()> {
    println!("Auth command - GitHub OAuth device flow (not yet implemented)");
    println!("This will:");
    println!("  1. Request device code from GitHub");
    println!("  2. Display verification URL and user code");
    println!("  3. Poll for access token");
    println!("  4. Store token in system keychain");
    Ok(())
}
