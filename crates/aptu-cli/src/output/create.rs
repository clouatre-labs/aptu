// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::CreateResult;

use super::Renderable;

impl Renderable for CreateResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        if self.dry_run {
            writeln!(
                w,
                "{}",
                style("DRY RUN - Issue not created").yellow().bold()
            )?;
        } else {
            writeln!(w, "{}", style("Issue Created Successfully").green().bold())?;
            writeln!(w, "  Number: {}", style(self.issue_number).cyan())?;
            writeln!(w, "  URL: {}", style(&self.issue_url).cyan().underlined())?;
        }
        writeln!(w)?;
        writeln!(w, "{}", style("Title").bold())?;
        writeln!(w, "  {}", self.title)?;
        writeln!(w)?;
        writeln!(w, "{}", style("Body").bold())?;
        for line in self.body.lines() {
            writeln!(w, "  {line}")?;
        }
        if !self.suggested_labels.is_empty() {
            writeln!(w)?;
            writeln!(w, "{}", style("Suggested Labels").bold())?;
            for label in &self.suggested_labels {
                writeln!(w, "  - {}", style(label).yellow())?;
            }
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Issue Created\n")?;
        writeln!(w, "**Title:** {}\n", self.title)?;
        writeln!(w, "**URL:** {}\n", self.issue_url)?;
        if !self.suggested_labels.is_empty() {
            writeln!(
                w,
                "**Suggested Labels:** {}\n",
                self.suggested_labels.join(", ")
            )?;
        }
        writeln!(w, "### Description\n")?;
        writeln!(w, "{}", self.body)?;
        Ok(())
    }
}

// Special handling for CreateResult to use custom JSON/YAML structure
impl CreateResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "issue_url": self.issue_url,
                    "issue_number": self.issue_number,
                    "title": self.title,
                    "body": self.body,
                    "suggested_labels": self.suggested_labels,
                    "dry_run": self.dry_run,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).expect("Failed to serialize create result")
                );
            }
            OutputFormat::Yaml => {
                let yaml = serde_yml::to_string(&serde_json::json!({
                    "issue_url": self.issue_url,
                    "issue_number": self.issue_number,
                    "title": self.title,
                    "body": self.body,
                    "suggested_labels": self.suggested_labels,
                    "dry_run": self.dry_run,
                }))
                .expect("Failed to serialize create result");
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx);
            }
        }
    }
}
