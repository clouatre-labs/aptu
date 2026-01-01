// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::models::ModelsResult;

use super::Renderable;

impl Renderable for ModelsResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(
            w,
            "{}",
            style(format!("Models from {}:", self.provider)).bold()
        )?;
        writeln!(w)?;

        if self.models.is_empty() {
            writeln!(w, "  {}", style("No models found").dim())?;
        } else {
            for (i, model) in self.models.iter().enumerate() {
                let num = format!("{:>3}.", i + 1);
                let id = format!("{:<30}", model.id);
                let name = model
                    .name
                    .as_deref()
                    .map_or_else(|| "N/A".to_string(), |n| format!("{n:<20}"));

                let free_str = match model.is_free {
                    Some(true) => style("free").green().to_string(),
                    Some(false) => style("paid").red().to_string(),
                    None => style("unknown").dim().to_string(),
                };

                let context_str = model
                    .context_window
                    .map_or_else(|| "N/A".to_string(), |cw| format!("{cw} tokens"));

                writeln!(
                    w,
                    "  {} {} {} {} {}",
                    style(num).dim(),
                    style(id).cyan(),
                    style(name).yellow(),
                    free_str,
                    style(context_str).dim()
                )?;
            }
        }

        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Models from {}\n", self.provider)?;

        if self.models.is_empty() {
            writeln!(w, "No models found.")?;
        } else {
            writeln!(w, "| ID | Name | Free | Context Window |")?;
            writeln!(w, "|---|---|---|---|")?;

            for model in &self.models {
                let name = model.name.as_deref().unwrap_or("N/A");
                let free = match model.is_free {
                    Some(true) => "Yes",
                    Some(false) => "No",
                    None => "Unknown",
                };
                let context = model
                    .context_window
                    .map_or_else(|| "N/A".to_string(), |cw| format!("{cw} tokens"));

                writeln!(w, "| {} | {} | {} | {} |", model.id, name, free, context)?;
            }
        }

        Ok(())
    }
}
