// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::BulkTriageResult;

use super::Renderable;

impl Renderable for BulkTriageResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Bulk Triage Summary").bold().green())?;
        writeln!(w, "{}", style("=".repeat(20)).dim())?;
        writeln!(w, "  Succeeded: {}", style(self.succeeded).green())?;
        writeln!(w, "  Failed:    {}", style(self.failed).red())?;
        writeln!(w, "  Skipped:   {}", style(self.skipped).yellow())?;
        writeln!(
            w,
            "  Total:     {}",
            self.succeeded + self.failed + self.skipped
        )?;
        writeln!(w)?;
        Ok(())
    }
}
