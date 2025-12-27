// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::ReposResult;

use super::Renderable;

impl Renderable for ReposResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Available repositories:").bold())?;
        writeln!(w)?;

        for (i, repo) in self.repos.iter().enumerate() {
            let num = format!("{:>3}.", i + 1);
            let name = format!("{:<25}", repo.full_name());
            let lang = format!("{:<10}", repo.language);

            writeln!(
                w,
                "  {} {} {} {}",
                style(num).dim(),
                style(name).cyan(),
                style(lang).yellow(),
                style(&repo.description).dim()
            )?;
        }

        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Available Repositories\n")?;
        for repo in &self.repos {
            writeln!(
                w,
                "- **{}** ({}) - {}",
                repo.full_name(),
                repo.language,
                &repo.description
            )?;
        }
        Ok(())
    }
}

// Special handling for ReposResult to maintain backward compatibility with JSON output
impl ReposResult {
    pub fn render_with_context(&self, ctx: &OutputContext) {
        match ctx.format {
            OutputFormat::Json => {
                // Output just the repos array for backward compatibility
                println!(
                    "{}",
                    serde_json::to_string_pretty(&self.repos)
                        .expect("Failed to serialize repos to JSON")
                );
            }
            OutputFormat::Yaml => {
                // Output just the repos array for backward compatibility
                println!(
                    "{}",
                    serde_saphyr::to_string(&self.repos)
                        .expect("Failed to serialize repos to YAML")
                );
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx);
            }
        }
    }
}
