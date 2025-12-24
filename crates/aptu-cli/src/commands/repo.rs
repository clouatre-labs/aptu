// SPDX-License-Identifier: Apache-2.0

//! List curated repositories command.

use anyhow::Result;
use aptu_core::list_curated_repos;

use super::types::ReposResult;

/// List curated repositories available for contribution.
pub async fn run() -> Result<ReposResult> {
    let repos = list_curated_repos().await?;
    Ok(ReposResult { repos })
}
