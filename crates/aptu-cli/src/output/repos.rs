// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use console::style;
use std::io::{self, Write};

use crate::cli::{OutputContext, OutputFormat};
use crate::commands::types::{DiscoverResult, ReposResult};

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

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Discovered Repositories\n")?;
        for repo in &self.repos {
            writeln!(
                w,
                "- **{}** ({} stars, score: {}) - [{}]({})",
                repo.full_name(),
                repo.stars,
                repo.score,
                repo.language.as_deref().unwrap_or("Unknown"),
                repo.url
            )?;
            if let Some(desc) = &repo.description {
                writeln!(w, "  - {desc}")?;
            }
        }
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
            OutputFormat::Yaml => {
                // Output just the repos array for backward compatibility
                let yaml = serde_saphyr::to_string(&self.repos)
                    .context("Failed to serialize repos to YAML")?;
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx)?;
            }
        }
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
            OutputFormat::Yaml => {
                // Output just the repos array for backward compatibility
                let yaml = serde_saphyr::to_string(&self.repos)
                    .context("Failed to serialize repos to YAML")?;
                println!("{yaml}");
            }
            _ => {
                // Use the trait implementation for text/markdown
                super::render(self, ctx)?;
            }
        }
        Ok(())
    }
}
