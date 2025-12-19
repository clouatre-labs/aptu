//! Aptu - Gamified OSS issue triage with AI assistance.
//!
//! A CLI tool that helps developers contribute meaningfully to open source
//! projects through AI-assisted issue triage and PR review.

mod auth;
mod cli;
mod commands;
mod errors;
mod logging;
mod output;

use anyhow::{Context, Result};
use aptu_core::config;
use clap::Parser;
use tracing::debug;

use crate::cli::{Cli, OutputContext};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init_logging(cli.quiet);

    let output_ctx = OutputContext::from_cli(cli.output, cli.quiet);

    // Load config early to validate it works (Option A from plan)
    let config = config::load_config().context("Failed to load configuration")?;
    debug!("Configuration loaded successfully");

    match commands::run(cli.command, output_ctx, &config).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let formatted = errors::format_error(&e);
            eprintln!("Error: {formatted}");
            Err(e)
        }
    }
}
