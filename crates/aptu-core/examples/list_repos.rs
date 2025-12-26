// SPDX-License-Identifier: Apache-2.0

//! List curated repositories using the async facade API.
//!
//! Run with: `cargo run --example list_repos -p aptu-core`

use aptu_core::list_curated_repos;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let repos = list_curated_repos().await?;

    println!("Found {} curated repositories:", repos.len());
    for repo in &repos {
        println!("  - {} ({})", repo.full_name(), repo.language);
    }

    Ok(())
}
