// SPDX-License-Identifier: Apache-2.0

//! Aptu - Gamified OSS issue triage with AI assistance.
//!
//! A CLI tool that helps developers contribute meaningfully to open source
//! projects through AI-assisted issue triage and PR review.

mod cli;
mod commands;
mod errors;
mod logging;
mod output;
mod provider;

pub use provider::CliTokenProvider;

use anyhow::{Context, Result};
use aptu_core::ai::registry;
use aptu_core::config;
use clap::Parser;
use tracing::debug;

use crate::cli::{Cli, OutputContext};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init_logging(cli.output, cli.verbose);

    let output_ctx = OutputContext::from_cli(cli.output, cli.verbose);

    // Load config early to validate it works (Option A from plan)
    let mut config = config::load_config().context("Failed to load configuration")?;
    debug!("Configuration loaded successfully");

    // Apply CLI overrides to config
    if let Some(provider) = &cli.provider {
        registry::get_provider(provider)
            .ok_or_else(|| anyhow::anyhow!("Unknown AI provider: {provider}"))?;
        config.ai.provider.clone_from(provider);
        debug!("Overriding AI provider to: {provider}");
    }

    if let Some(model) = &cli.model {
        config.ai.model.clone_from(model);
        debug!("Overriding AI model to: {model}");
    }

    match commands::run(cli.command, output_ctx, &config).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let formatted = errors::format_error(&e);
            eprintln!("Error: {formatted}");
            Err(e)
        }
    }
}
