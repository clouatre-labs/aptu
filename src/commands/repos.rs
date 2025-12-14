//! List curated repositories command.

use anyhow::Result;

/// List curated repositories available for contribution.
pub async fn run() -> Result<()> {
    println!("Repos command - list curated repositories (not yet implemented)");
    println!("This will display repositories known to be:");
    println!("  - Active (commits in last 30 days)");
    println!("  - Welcoming (good first issue labels)");
    println!("  - Responsive (maintainers reply within 1 week)");
    Ok(())
}
