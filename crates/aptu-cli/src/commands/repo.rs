//! List curated repositories command.

use aptu_core::repos;

use super::types::ReposResult;

/// List curated repositories available for contribution.
pub fn run() -> ReposResult {
    let repos = repos::list();
    ReposResult { repos }
}
