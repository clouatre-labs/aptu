// SPDX-License-Identifier: Apache-2.0

//! Output rendering for models command.

use std::io::{self, Write};

use console::style;

use crate::cli::OutputContext;
use crate::commands::types::ModelsResult;
use crate::output::Renderable;

impl Renderable for ModelsResult {
    fn render_text(&self, w: &mut dyn Write, ctx: &OutputContext) -> io::Result<()> {
        writeln!(
            w,
            "{}",
            style(format!("Models from {}", self.provider)).bold()
        )?;
        writeln!(w)?;

        if self.models.is_empty() {
            writeln!(w, "  {}", style("No models available").dim())?;
            return Ok(());
        }

        for model in &self.models {
            let free_indicator = if model.is_free {
                style("✓ FREE").green().to_string()
            } else {
                style("  PAID").dim().to_string()
            };

            writeln!(
                w,
                "  {} {}",
                free_indicator,
                style(&model.display_name).cyan()
            )?;
            writeln!(
                w,
                "      {} ({}k tokens)",
                style(&model.identifier).dim(),
                model.context_window / 1000
            )?;
        }

        let _ = ctx;
        Ok(())
    }
}
