//! List curated repositories command.

use anyhow::Result;
use console::style;

use crate::repos;

/// List curated repositories available for contribution.
pub async fn run() -> Result<()> {
    let repos = repos::list();

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

    Ok(())
}
