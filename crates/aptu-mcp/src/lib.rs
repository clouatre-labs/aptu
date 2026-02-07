// SPDX-License-Identifier: Apache-2.0

//! MCP server exposing aptu-core functionality for AI-powered GitHub triage and review.
//!
//! This crate provides an MCP (Model Context Protocol) server that wraps aptu-core
//! facade functions as MCP tools, resources, and prompts. It uses the RMCP Rust SDK
//! with stdio transport for integration with MCP-compatible clients.

mod auth;
mod error;
mod server;

pub use server::{AptuServer, CredentialStatus, HealthCheckParams, HealthCheckResponse};

/// Run the MCP server over stdio transport.
///
/// Serves the MCP protocol over stdin/stdout.
pub async fn run_stdio() -> anyhow::Result<()> {
    use rmcp::{ServiceExt, transport::stdio};

    tracing::info!("Starting aptu MCP server (stdio)");

    let server = AptuServer::new();
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("Server error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}

/// Run the MCP server over HTTP transport.
///
/// Starts an HTTP server on the specified host and port, serving the MCP protocol
/// at the /mcp endpoint. Gracefully shuts down on Ctrl+C.
pub async fn run_http(host: &str, port: u16) -> anyhow::Result<()> {
    use axum::Router;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    tracing::info!("Starting aptu MCP HTTP server on {}:{}", host, port);

    let session_manager = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default();

    let service = StreamableHttpService::new(
        || {
            let server = AptuServer::new();
            Ok(server)
        },
        session_manager,
        config,
    );

    let router = Router::new().nest_service("/mcp", service);

    // Handle both IPv4 and IPv6 addresses
    let addr: SocketAddr = if host.contains(':') {
        // IPv6 address - needs brackets
        format!("[{host}]:{port}")
    } else {
        // IPv4 address or hostname
        format!("{host}:{port}")
    }
    .parse()?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("HTTP server listening on {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            tracing::info!("Received Ctrl+C, shutting down gracefully");
        })
        .await?;

    Ok(())
}
