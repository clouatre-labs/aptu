//! List curated repositories command.

use anyhow::Result;
use console::style;

use crate::cli::{OutputContext, OutputFormat};
use crate::repos;

/// List curated repositories available for contribution.
pub async fn run(ctx: OutputContext) -> Result<()> {
    let repos = repos::list();

    match ctx.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(repos)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yml::to_string(repos)?);
        }
        OutputFormat::Text => {
            println!();
            println!("{}", style("Available repositories:").bold());
            println!();

            for (i, repo) in repos.iter().enumerate() {
                let num = format!("{:>3}.", i + 1);
                let name = format!("{:<25}", repo.full_name());
                let lang = format!("{:<10}", repo.language);

                println!(
                    "  {} {} {} {}",
                    style(num).dim(),
                    style(name).cyan(),
                    style(lang).yellow(),
                    style(repo.description).dim()
                );
            }

            println!();
        }
    }

    Ok(())
}
