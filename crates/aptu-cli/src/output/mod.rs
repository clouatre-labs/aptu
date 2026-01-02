// SPDX-License-Identifier: Apache-2.0

//! Output rendering for CLI commands.
//!
//! Centralizes all output formatting logic, supporting text, JSON, and YAML formats.
//! Command handlers return data; this module handles presentation.

use anyhow::{Context, Result};
use serde::Serialize;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};

/// Trait for types that can be rendered in multiple output formats.
pub trait Renderable: Serialize {
    /// Render as human-readable text to the given writer.
    fn render_text(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()>;

    /// Render as markdown. Defaults to text rendering.
    fn render_markdown(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()> {
        self.render_text(w, ctx)
    }
}

/// Generic render function - handles JSON/YAML via serde, delegates text/markdown to trait.
pub fn render<T: Renderable>(result: &T, ctx: &OutputContext) -> Result<()> {
    match ctx.format {
        OutputFormat::Json => {
            let json =
                serde_json::to_string_pretty(result).context("Failed to serialize to JSON")?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_saphyr::to_string(result).context("Failed to serialize to YAML")?;
            println!("{yaml}");
        }
        OutputFormat::Markdown => {
            result
                .render_markdown(&mut io::stdout(), ctx)
                .context("Failed to render markdown")?;
        }
        OutputFormat::Text => {
            result
                .render_text(&mut io::stdout(), ctx)
                .context("Failed to render text")?;
        }
    }
    Ok(())
}

mod auth;
mod bulk;
pub mod common;
mod create;
mod history;
mod issues;
mod models;
mod pr;
mod release;
mod repos;
mod triage;
