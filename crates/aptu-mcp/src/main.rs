// SPDX-License-Identifier: Apache-2.0

//! Binary entry point for the aptu MCP server.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    aptu_mcp::run_stdio().await
}
