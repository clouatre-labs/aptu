//! List curated repositories command.

use super::types::ReposResult;
use crate::repos;

/// List curated repositories available for contribution.
pub fn run() -> ReposResult {
    let repos = repos::list();
    ReposResult { repos }
}
