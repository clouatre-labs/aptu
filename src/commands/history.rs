//! Contribution history command.

use anyhow::Result;

/// Show contribution history.
pub async fn run() -> Result<()> {
    println!("History command - contribution history (not yet implemented)");
    println!("This will display your local contribution log:");
    println!("  - Issues triaged");
    println!("  - Comments posted");
    println!("  - Status (pending/accepted/rejected)");
    Ok(())
}
