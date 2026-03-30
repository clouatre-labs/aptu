// SPDX-License-Identifier: Apache-2.0

//! MCP server exposing aptu-core functionality for AI-powered GitHub triage and review.
//!
//! This crate provides an MCP (Model Context Protocol) server that wraps aptu-core
//! facade functions as MCP tools, resources, and prompts. It uses the RMCP Rust SDK
//! with stdio transport for integration with MCP-compatible clients.

use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use std::sync::Arc;

mod auth;
mod error;
mod server;

pub use server::{AptuServer, CredentialStatus, HealthCheckParams, HealthCheckResponse};

/// Run the MCP server over stdio transport.
///
/// Serves the MCP protocol over stdin/stdout.
///
/// # Arguments
/// * `read_only` - If true, disables write tools (`post_triage`, `post_review`)
pub async fn run_stdio(read_only: bool) -> anyhow::Result<()> {
    use anyhow::Context;
    use rmcp::{ServiceExt, transport::stdio};

    tracing::info!("Starting aptu MCP server (stdio)");

    let ai_config = aptu_core::config::load_config()
        .context("Failed to load configuration")?
        .ai;

    let server = AptuServer::with_config(read_only, ai_config);
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
///
/// # Arguments
/// * `host` - Host to bind to
/// * `port` - Port to bind to
/// * `read_only` - If true, disables write tools (`post_triage`, `post_review`)
// Validate bearer token using constant-time comparison
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

fn validate_bearer(expected: &str, auth_header: Option<&str>) -> bool {
    match auth_header {
        Some(val) if val.starts_with("Bearer ") => {
            let token = &val[7..];
            constant_time_eq(token.as_bytes(), expected.as_bytes())
        }
        _ => false,
    }
}

async fn bearer_auth(expected: Arc<str>, req: Request, next: Next) -> Response {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if !validate_bearer(&expected, auth_header) {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "error": {"code": -32001, "message": "Unauthorized"},
            "id": null
        })
        .to_string();
        let mut response = Response::new(body.into());
        *response.status_mut() = StatusCode::UNAUTHORIZED;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        return response;
    }
    next.run(req).await
}

/// Apply bearer token middleware to `router` if `MCP_BEARER_TOKEN` is set and non-empty.
///
/// Logs a warning when the env var is absent or empty so operators notice an unprotected endpoint.
fn apply_bearer_middleware(router: axum::Router) -> axum::Router {
    match std::env::var("MCP_BEARER_TOKEN") {
        Ok(token) if !token.is_empty() => {
            let token = Arc::from(token.as_str());
            router.layer(middleware::from_fn(move |req, next| {
                bearer_auth(Arc::clone(&token), req, next)
            }))
        }
        Ok(_) => {
            tracing::warn!("MCP_BEARER_TOKEN is empty; HTTP endpoint is unauthenticated");
            router
        }
        Err(_) => {
            tracing::warn!("MCP_BEARER_TOKEN is not set; HTTP endpoint is unauthenticated");
            router
        }
    }
}

/// Parse `host:port` into a [`SocketAddr`], bracketing IPv6 addresses.
fn parse_socket_addr(host: &str, port: u16) -> anyhow::Result<std::net::SocketAddr> {
    let addr_str = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    addr_str.parse().map_err(Into::into)
}

pub async fn run_http(host: &str, port: u16, read_only: bool) -> anyhow::Result<()> {
    use anyhow::Context;
    use axum::Router;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    use std::sync::Arc;
    use tokio::net::TcpListener;

    tracing::info!("Starting aptu MCP HTTP server on {}:{}", host, port);

    let ai_config = aptu_core::config::load_config()
        .context("Failed to load configuration")?
        .ai;

    let session_manager = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default();

    let service = StreamableHttpService::new(
        move || {
            let server = AptuServer::with_config(read_only, ai_config.clone());
            Ok(server)
        },
        session_manager,
        config,
    );

    let router = Router::new().nest_service("/mcp", service);
    let router = apply_bearer_middleware(router);

    let addr = parse_socket_addr(host, port)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bearer_valid() {
        assert!(validate_bearer("secret123", Some("Bearer secret123")));
    }

    #[test]
    fn test_validate_bearer_missing_header() {
        assert!(!validate_bearer("secret123", None));
    }

    #[test]
    fn test_validate_bearer_wrong_token() {
        assert!(!validate_bearer("secret123", Some("Bearer wrongtoken")));
    }

    #[test]
    fn test_validate_bearer_wrong_scheme() {
        assert!(!validate_bearer("secret123", Some("Basic dXNlcjpwYXNz")));
    }

    #[test]
    fn test_constant_time_eq_equal() {
        assert!(constant_time_eq(b"abc", b"abc"));
    }

    #[test]
    fn test_constant_time_eq_different_length() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn test_constant_time_eq_different_content() {
        assert!(!constant_time_eq(b"abc", b"abd"));
    }
}
