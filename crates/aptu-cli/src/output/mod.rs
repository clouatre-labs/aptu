// SPDX-License-Identifier: Apache-2.0

//! Output rendering for CLI commands.
//!
//! Centralizes all output formatting logic, supporting text, JSON, and YAML formats.
//! Command handlers return data; this module handles presentation.

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
pub fn render<T: Renderable>(result: &T, ctx: &OutputContext) {
    match ctx.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("Failed to serialize to JSON")
            );
        }
        OutputFormat::Yaml => {
            println!(
                "{}",
                serde_saphyr::to_string(result).expect("Failed to serialize to YAML")
            );
        }
        OutputFormat::Markdown => {
            result
                .render_markdown(&mut io::stdout(), ctx)
                .expect("Failed to render markdown");
        }
        OutputFormat::Text => {
            result
                .render_text(&mut io::stdout(), ctx)
                .expect("Failed to render text");
        }
    }
}

mod auth;
mod bulk;
mod create;
mod history;
mod issues;
mod models;
mod pr;
mod release;
mod repos;
mod triage;
