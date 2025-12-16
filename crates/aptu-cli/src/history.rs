//! Local contribution history tracking.
//!
//! Stores contribution records in `~/.local/share/aptu/history.json`.
//! Each contribution tracks repo, issue, action, timestamp, and status.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::data_dir;

/// Status of a contribution.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContributionStatus {
    /// Contribution submitted, awaiting maintainer response.
    #[default]
    Pending,
    /// Maintainer accepted the contribution.
    Accepted,
    /// Maintainer rejected the contribution.
    Rejected,
}

/// A single contribution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    /// Unique identifier.
    pub id: Uuid,
    /// Repository in "owner/repo" format.
    pub repo: String,
    /// Issue number.
    pub issue: u64,
    /// Action type (e.g., "triage").
    pub action: String,
    /// When the contribution was made.
    pub timestamp: DateTime<Utc>,
    /// URL to the posted comment.
    pub comment_url: String,
    /// Current status of the contribution.
    #[serde(default)]
    pub status: ContributionStatus,
}

/// Container for all contribution history.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HistoryData {
    /// List of contributions.
    pub contributions: Vec<Contribution>,
}

/// Returns the path to the history file.
pub fn history_file_path() -> PathBuf {
    data_dir().join("history.json")
}

/// Load contribution history from disk.
///
/// Returns empty history if file doesn't exist.
pub fn load() -> Result<HistoryData> {
    let path = history_file_path();

    if !path.exists() {
        return Ok(HistoryData::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read history file: {}", path.display()))?;

    let data: HistoryData = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse history file: {}", path.display()))?;

    Ok(data)
}

/// Save contribution history to disk.
///
/// Creates parent directories if they don't exist.
pub fn save(data: &HistoryData) -> Result<()> {
    let path = history_file_path();

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let contents =
        serde_json::to_string_pretty(data).context("Failed to serialize history data")?;

    fs::write(&path, contents)
        .with_context(|| format!("Failed to write history file: {}", path.display()))?;

    Ok(())
}

/// Add a contribution to history.
///
/// Loads existing history, appends the new contribution, and saves.
pub fn add_contribution(contribution: Contribution) -> Result<()> {
    let mut data = load()?;
    data.contributions.push(contribution);
    save(&data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test contribution.
    fn test_contribution() -> Contribution {
        Contribution {
            id: Uuid::new_v4(),
            repo: "owner/repo".to_string(),
            issue: 123,
            action: "triage".to_string(),
            timestamp: Utc::now(),
            comment_url: "https://github.com/owner/repo/issues/123#issuecomment-1".to_string(),
            status: ContributionStatus::Pending,
        }
    }

    #[test]
    fn test_contribution_serialization_roundtrip() {
        let contribution = test_contribution();
        let json = serde_json::to_string(&contribution).expect("serialize");
        let parsed: Contribution = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(contribution.id, parsed.id);
        assert_eq!(contribution.repo, parsed.repo);
        assert_eq!(contribution.issue, parsed.issue);
        assert_eq!(contribution.action, parsed.action);
        assert_eq!(contribution.comment_url, parsed.comment_url);
        assert_eq!(contribution.status, parsed.status);
    }

    #[test]
    fn test_history_data_serialization_roundtrip() {
        let data = HistoryData {
            contributions: vec![test_contribution(), test_contribution()],
        };

        let json = serde_json::to_string_pretty(&data).expect("serialize");
        let parsed: HistoryData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.contributions.len(), 2);
    }

    #[test]
    fn test_contribution_status_default() {
        let status = ContributionStatus::default();
        assert_eq!(status, ContributionStatus::Pending);
    }

    #[test]
    fn test_contribution_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ContributionStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ContributionStatus::Accepted).unwrap(),
            "\"accepted\""
        );
        assert_eq!(
            serde_json::to_string(&ContributionStatus::Rejected).unwrap(),
            "\"rejected\""
        );
    }

    #[test]
    fn test_empty_history_default() {
        let data = HistoryData::default();
        assert!(data.contributions.is_empty());
    }
}
