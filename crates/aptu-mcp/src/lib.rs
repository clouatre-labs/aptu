// SPDX-License-Identifier: Apache-2.0

//! MCP server exposing aptu-core functionality for AI-powered GitHub triage and review.
//!
//! This crate provides an MCP (Model Context Protocol) server that wraps aptu-core
//! facade functions as MCP tools, resources, and prompts. It uses the RMCP Rust SDK
//! with stdio transport for integration with MCP-compatible clients.

mod auth;
mod error;
mod server;

pub use server::AptuServer;

/// Run the MCP server over stdio transport.
///
/// Attempts to initialize tracing to stderr and serves the MCP protocol over stdin/stdout.
pub async fn run_stdio() -> anyhow::Result<()> {
    use rmcp::{ServiceExt, transport::stdio};
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init()
        .ok();

    tracing::info!("Starting aptu MCP server");

    let server = AptuServer::new();
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("Server error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
