// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::{DiscoverResult, RepoMutateResult, ReposResult};

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
}

impl Renderable for DiscoverResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", style("Discovered repositories:").bold())?;
        writeln!(w)?;

        for (i, repo) in self.repos.iter().enumerate() {
            let num = format!("{:>3}.", i + 1);
            let name = format!("{:<25}", repo.full_name());
            let stars = format!("{:>5} stars", repo.stars);
            let score = format!("score: {}", repo.score);

            writeln!(
                w,
                "  {} {} {} {}",
                style(num).dim(),
                style(name).cyan(),
                style(stars).yellow(),
                style(score).green()
            )?;

            if let Some(lang) = &repo.language {
                writeln!(w, "     Language: {}", style(lang).dim())?;
            }

            if let Some(desc) = &repo.description {
                writeln!(w, "     {}", style(desc).dim())?;
            }

            writeln!(w, "     {}", style(&repo.url).blue())?;
        }

        writeln!(w)?;
        Ok(())
    }
}

// Special handling for ReposResult to maintain backward compatibility with JSON output
impl ReposResult {
    pub fn render_with_context(&self, ctx: &OutputContext) -> Result<()> {
        match ctx.format {
            OutputFormat::Json => {
                // Output just the repos array for backward compatibility
                let json = serde_json::to_string_pretty(&self.repos)
                    .context("Failed to serialize repos to JSON")?;
                println!("{json}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx)?;
            }
        }
        Ok(())
    }
}

impl Renderable for RepoMutateResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "{} {}", style("*").green().bold(), self.message)?;
        Ok(())
    }
}

// Special handling for DiscoverResult to maintain backward compatibility with JSON output
impl DiscoverResult {
    pub fn render_with_context(&self, ctx: &OutputContext) -> Result<()> {
        match ctx.format {
            OutputFormat::Json => {
                // Output just the repos array for backward compatibility
                let json = serde_json::to_string_pretty(&self.repos)
                    .context("Failed to serialize repos to JSON")?;
                println!("{json}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx)?;
            }
        }
        Ok(())
    }
}
