//! Aptu - Gamified OSS issue triage with AI assistance.
//!
//! A CLI tool that helps developers contribute meaningfully to open source
//! projects through AI-assisted issue triage and PR review.

mod ai;
mod cli;
mod commands;
mod config;
mod error;
mod github;
mod logging;
mod repos;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::debug;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    logging::init_logging();

    let cli = Cli::parse();

    // Load config early to validate it works (Option A from plan)
    #[allow(unused_variables)]
    let config = config::load_config().context("Failed to load configuration")?;
    debug!("Configuration loaded successfully");

    commands::run(cli.command).await
}
