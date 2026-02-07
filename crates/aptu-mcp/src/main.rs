// SPDX-License-Identifier: Apache-2.0

//! Binary entry point for the aptu MCP server.

use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "aptu-mcp")]
#[command(
    about = "MCP server exposing aptu-core functionality for AI-powered GitHub triage and review"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Transport mode (stdio or http)
    #[arg(long, default_value = "stdio")]
    transport: Transport,

    /// Host to bind to (HTTP mode only)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to (HTTP mode only)
    #[arg(long, default_value = "3000")]
    port: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Subcommand)]
enum Command {
    /// Run the MCP server (default)
    Run,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing once at the entry point
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init()
        .ok();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run) | None => match cli.transport {
            Transport::Stdio => aptu_mcp::run_stdio().await,
            Transport::Http => aptu_mcp::run_http(&cli.host, cli.port).await,
        },
    }
}
