//! Contribution history command.

use anyhow::Result;
use aptu_core::history;

use super::types::HistoryResult;

/// Show contribution history.
pub fn run() -> Result<HistoryResult> {
    let data = history::load()?;
    Ok(HistoryResult {
        contributions: data.contributions,
    })
}
