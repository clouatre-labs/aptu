//! List open issues command.

use anyhow::Result;

/// List open issues suitable for contribution.
pub async fn run(repo: Option<String>) -> Result<()> {
    println!("Issues command - list open issues (not yet implemented)");
    if let Some(r) = repo {
        println!("  Filtering by repository: {}", r);
    } else {
        println!("  Showing issues from all curated repositories");
    }
    println!("Filters applied:");
    println!("  - Label: good first issue OR help wanted");
    println!("  - State: Open, no assignee");
    println!("  - Created in last 90 days");
    Ok(())
}
