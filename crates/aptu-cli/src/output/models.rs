// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::models::{ModelsResult, ModelsResultMulti, SerializableModelInfo};

use super::Renderable;

/// Compute the display width for the id column based on the longest id in the list.
fn id_col_width(models: &[SerializableModelInfo]) -> usize {
    models
        .iter()
        .map(|m| m.id.len())
        .max()
        .unwrap_or(20)
        .max(20)
}

/// Compute the display width for the name column based on the longest name in the list.
fn name_col_width(models: &[SerializableModelInfo]) -> usize {
    models
        .iter()
        .map(|m| m.name.as_deref().unwrap_or("").len())
        .max()
        .unwrap_or(0)
        .max(4) // minimum "Name" header width
}

/// Returns true when every model has no pricing information (all `is_free` are `None`).
///
/// Called once per render pass (not in a tight loop), so a single linear scan
/// over the model list is acceptable.
fn all_unknown(models: &[SerializableModelInfo]) -> bool {
    models.iter().all(|m| m.is_free.is_none())
}

/// Write a single model row to the writer.
fn write_model_row(
    w: &mut dyn Write,
    index: usize,
    model: &SerializableModelInfo,
    id_width: usize,
    name_width: usize,
    show_pricing: bool,
) -> io::Result<()> {
    let num = format!("{:>3}.", index + 1);
    let id = format!("{:<width$}", model.id, width = id_width);
    let name = model
        .name
        .as_deref()
        .map_or_else(|| "N/A".to_string(), |n| format!("{n:<name_width$}"));

    let context_str = model
        .context_window
        .map_or_else(|| "N/A".to_string(), |cw| format!("{cw} tokens"));

    if show_pricing {
        let free_str = match model.is_free {
            Some(true) => style("free").green().to_string(),
            Some(false) => style("paid").red().to_string(),
            None => style("unknown").dim().to_string(),
        };
        writeln!(
            w,
            "  {} {} {} {} {}",
            style(num).dim(),
            style(id).cyan(),
            style(name).yellow(),
            free_str,
            style(context_str).dim()
        )
    } else {
        writeln!(
            w,
            "  {} {} {} {}",
            style(num).dim(),
            style(id).cyan(),
            style(name).yellow(),
            style(context_str).dim()
        )
    }
}

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
            let id_width = id_col_width(&self.models);
            let name_w = name_col_width(&self.models);
            let show_pricing = !all_unknown(&self.models);
            for (i, model) in self.models.iter().enumerate() {
                write_model_row(w, i, model, id_width, name_w, show_pricing)?;
            }
            let count = self.models.len();
            writeln!(
                w,
                "  {}",
                style(format!(
                    "{} model{}",
                    count,
                    if count == 1 { "" } else { "s" }
                ))
                .dim()
            )?;
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

impl Renderable for ModelsResultMulti {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Available AI Models").bold())?;
        writeln!(w)?;

        if self.results.is_empty() {
            writeln!(w, "  {}", style("No models found").dim())?;
        } else {
            for result in &self.results {
                writeln!(
                    w,
                    "{}",
                    style(format!("{}:", result.provider)).bold().cyan()
                )?;
                writeln!(w)?;

                if result.models.is_empty() {
                    writeln!(w, "  {}", style("No models found").dim())?;
                } else {
                    let id_width = id_col_width(&result.models);
                    let name_w = name_col_width(&result.models);
                    let show_pricing = !all_unknown(&result.models);
                    for (i, model) in result.models.iter().enumerate() {
                        write_model_row(w, i, model, id_width, name_w, show_pricing)?;
                    }
                    let count = result.models.len();
                    writeln!(
                        w,
                        "  {}",
                        style(format!(
                            "{} model{}",
                            count,
                            if count == 1 { "" } else { "s" }
                        ))
                        .dim()
                    )?;
                }
                writeln!(w)?;
            }
        }

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "# Available AI Models\n")?;

        if self.results.is_empty() {
            writeln!(w, "No models found.")?;
        } else {
            for result in &self.results {
                writeln!(w, "## {}\n", result.provider)?;

                if result.models.is_empty() {
                    writeln!(w, "No models found.\n")?;
                } else {
                    writeln!(w, "| ID | Name | Free | Context Window |")?;
                    writeln!(w, "|---|---|---|---|")?;

                    for model in &result.models {
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
                    writeln!(w)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model(id: &str, is_free: Option<bool>) -> SerializableModelInfo {
        SerializableModelInfo {
            id: id.to_string(),
            name: None,
            is_free,
            context_window: None,
            provider: "test".to_string(),
            capabilities: vec![],
            pricing: None,
        }
    }

    #[test]
    fn test_all_unknown_all_none() {
        let models = vec![make_model("a", None), make_model("b", None)];
        assert!(all_unknown(&models));
    }

    #[test]
    fn test_all_unknown_mixed() {
        let models = vec![make_model("a", None), make_model("b", Some(true))];
        assert!(!all_unknown(&models));
    }
}
