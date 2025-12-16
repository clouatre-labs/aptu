//! Contribution history command.

use anyhow::Result;

use super::types::HistoryResult;
use crate::history;

/// Show contribution history.
pub async fn run() -> Result<HistoryResult> {
    let data = history::load()?;
    Ok(HistoryResult {
        contributions: data.contributions,
    })
}
