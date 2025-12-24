// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
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

// Special handling for BulkTriageResult to use custom JSON/YAML structure
impl BulkTriageResult {
    #[allow(dead_code)]
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                let summary = serde_json::json!({
                    "succeeded": self.succeeded,
                    "failed": self.failed,
                    "skipped": self.skipped,
                    "total": self.succeeded + self.failed + self.skipped,
                    "results": self.outcomes.iter().map(|(repo, outcome)| {
                        serde_json::json!({
                            "repository": repo,
                            "outcome": outcome,
                        })
                    }).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("Failed to serialize summary")
                );
            }
            OutputFormat::Yaml => {
                let summary = serde_json::json!({
                    "succeeded": self.succeeded,
                    "failed": self.failed,
                    "skipped": self.skipped,
                    "total": self.succeeded + self.failed + self.skipped,
                    "results": self.outcomes.iter().map(|(repo, outcome)| {
                        serde_json::json!({
                            "repository": repo,
                            "outcome": outcome,
                        })
                    }).collect::<Vec<_>>(),
                });
                let yaml = serde_yml::to_string(&summary).expect("Failed to serialize summary");
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx);
            }
        }
    }
}
