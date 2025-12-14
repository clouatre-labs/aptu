//! Command handlers for Aptu CLI.

pub mod auth;
pub mod history;
pub mod issues;
pub mod repos;
pub mod triage;

use anyhow::Result;

use crate::cli::Commands;

/// Dispatch to the appropriate command handler.
pub async fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Auth => auth::run().await,
        Commands::Repos => repos::run().await,
        Commands::Issues { repo } => issues::run(repo).await,
        Commands::Triage { issue_url } => triage::run(issue_url).await,
        Commands::History => history::run().await,
    }
}
