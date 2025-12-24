// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
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
