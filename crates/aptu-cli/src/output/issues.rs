// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::RevertResult;

use super::Renderable;

impl Renderable for RevertResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style(&self.summary).bold())?;
        writeln!(w)?;

        if !self.labels_removed.is_empty() {
            writeln!(w, "{}:", style("Labels removed").cyan())?;
            for label in &self.labels_removed {
                writeln!(w, "  - {label}")?;
            }
        }

        if !self.comment_ids.is_empty() {
            writeln!(w, "{}:", style("Comments removed").cyan())?;
            for comment_id in &self.comment_ids {
                writeln!(w, "  - #{comment_id}")?;
            }
        }

        if self.dry_run {
            writeln!(
                w,
                "{}",
                style("(dry-run: no changes were made)").italic().dim()
            )?;
        }

        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## {}", self.summary)?;
        writeln!(w)?;

        if !self.labels_removed.is_empty() {
            writeln!(w, "### Labels Removed")?;
            writeln!(w)?;
            for label in &self.labels_removed {
                writeln!(w, "- `{label}`")?;
            }
            writeln!(w)?;
        }

        if !self.comment_ids.is_empty() {
            writeln!(w, "### Comments Removed")?;
            writeln!(w)?;
            for comment_id in &self.comment_ids {
                writeln!(w, "- #{comment_id}")?;
            }
            writeln!(w)?;
        }

        if self.dry_run {
            writeln!(w, "**(dry-run: no changes were made)**")?;
        }

        Ok(())
    }
    // Note: JSON output is handled by the generic Renderable impl via serde serialization.
    // RevertResult derives Serialize, so serde_json::to_string_pretty is used automatically
    // in the render function (crates/aptu-cli/src/output/mod.rs:30).
}
