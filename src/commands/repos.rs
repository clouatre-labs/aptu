//! List curated repositories command.

use anyhow::Result;

use super::types::ReposResult;
use crate::repos;

/// List curated repositories available for contribution.
pub async fn run() -> Result<ReposResult> {
    let repos = repos::list();
    Ok(ReposResult { repos })
}
