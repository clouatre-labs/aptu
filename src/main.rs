//! Aptu - Gamified OSS issue triage with AI assistance.
//!
//! A CLI tool that helps developers contribute meaningfully to open source
//! projects through AI-assisted issue triage and PR review.

mod config;
mod error;
mod logging;

use anyhow::Result;
use tracing::info;

fn main() -> Result<()> {
    logging::init_logging();

    info!("Aptu starting...");
    println!("Hello, world!");

    Ok(())
}
