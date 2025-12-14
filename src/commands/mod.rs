//! Command handlers for Aptu CLI.

pub mod auth;
pub mod history;
pub mod issues;
pub mod repos;
pub mod triage;

use anyhow::Result;

use crate::cli::{Commands, OutputContext};

/// Dispatch to the appropriate command handler.
pub async fn run(command: Commands, ctx: OutputContext) -> Result<()> {
    match command {
        Commands::Auth { logout } => auth::run(logout).await,
        Commands::Repos => repos::run(ctx).await,
        Commands::Issues { repo } => issues::run(repo, ctx).await,
        Commands::Triage {
            issue_url,
            dry_run,
            yes,
        } => triage::run(issue_url, dry_run, yes, ctx).await,
        Commands::History => history::run().await,
    }
}
