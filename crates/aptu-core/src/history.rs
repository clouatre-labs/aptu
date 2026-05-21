// SPDX-License-Identifier: Apache-2.0

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

// ETU weight constants. These are the structural Anthropic cache pricing ratios;
// they belong here alongside AiStats rather than in the provider layer.
const ETU_WEIGHT_INPUT: f64 = 1.0;
/// Cache-read tokens cost 0.1× input price (90% discount). Stable since Claude 3.
const ETU_WEIGHT_CACHE_READ: f64 = 0.1;
/// Cache-write tokens cost 1.25× input price (5-min TTL). Confirmed May 2026.
const ETU_WEIGHT_CACHE_WRITE: f64 = 1.25;
/// Output tokens cost 5× input price across all current models. Stable since Claude 3.
const ETU_WEIGHT_OUTPUT: f64 = 5.0;

/// Compute Effective Token Units from raw token counts.
///
/// ETU = 1.0·input + 0.1·`cache_read` + 1.25·`cache_write` + 5.0·output
///
/// Weights are structural Anthropic cache pricing ratios (not per-model prices),
/// stable across all model generations since Claude 3. No pricing table needed.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn compute_etu(input: u64, cache_read: u64, cache_write: u64, output: u64) -> f64 {
    ETU_WEIGHT_INPUT * input as f64
        + ETU_WEIGHT_CACHE_READ * cache_read as f64
        + ETU_WEIGHT_CACHE_WRITE * cache_write as f64
        + ETU_WEIGHT_OUTPUT * output as f64
}

/// AI usage statistics for a contribution.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct AiStats {
    /// Provider name (e.g., "openrouter", "anthropic").
    pub provider: String,
    /// Model used for analysis.
    pub model: String,
    /// Number of input tokens.
    pub input_tokens: u64,
    /// Number of output tokens.
    pub output_tokens: u64,
    /// Duration of the API call in milliseconds.
    pub duration_ms: u64,
    /// Cost in USD (from `OpenRouter` API, `None` if not reported).
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// Fallback provider used if primary failed (None if primary succeeded).
    #[serde(default)]
    pub fallback_provider: Option<String>,
    /// Prompt size in characters.
    #[serde(default)]
    pub prompt_chars: usize,
    /// Number of cache read tokens (from Anthropic API).
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Number of cache write tokens (from Anthropic API).
    #[serde(default)]
    pub cache_write_tokens: u64,
    /// Effective Token Units: a normalized throughput signal comparable across operations.
    /// Computed via [`compute_etu`]; see that function for the formula and weight rationale.
    #[serde(default)]
    pub effective_token_units: f64,
    /// Trace ID for correlating with context records (optional, not serialized if None).
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl AiStats {
    /// Recompute and set `effective_token_units` from the current token counts.
    ///
    /// Call at the end of any construction chain to ensure ETU stays consistent
    /// with the token fields rather than being set manually at each site.
    #[must_use]
    pub fn with_computed_etu(mut self) -> Self {
        self.effective_token_units = compute_etu(
            self.input_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
            self.output_tokens,
        );
        self
    }
}

impl<'de> Deserialize<'de> for AiStats {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default)]
            provider: String,
            #[serde(default)]
            model: String,
            #[serde(default)]
            input_tokens: u64,
            #[serde(default)]
            output_tokens: u64,
            #[serde(default)]
            duration_ms: u64,
            #[serde(default)]
            cost_usd: Option<f64>,
            #[serde(default)]
            fallback_provider: Option<String>,
            #[serde(default)]
            prompt_chars: usize,
            #[serde(default)]
            cache_read_tokens: u64,
            #[serde(default)]
            cache_write_tokens: u64,
            /// Ignored on deserialise; recomputed in the From impl.
            #[serde(default)]
            #[allow(dead_code)]
            effective_token_units: f64,
            #[serde(default)]
            trace_id: Option<String>,
        }

        let h = Helper::deserialize(deserializer)?;
        Ok(AiStats {
            provider: h.provider,
            model: h.model,
            input_tokens: h.input_tokens,
            output_tokens: h.output_tokens,
            duration_ms: h.duration_ms,
            cost_usd: h.cost_usd,
            fallback_provider: h.fallback_provider,
            prompt_chars: h.prompt_chars,
            cache_read_tokens: h.cache_read_tokens,
            cache_write_tokens: h.cache_write_tokens,
            effective_token_units: 0.0,
            trace_id: h.trace_id,
        }
        .with_computed_etu())
    }
}

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
    /// AI usage statistics for this contribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_stats: Option<AiStats>,
}

/// Container for all contribution history.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HistoryData {
    /// List of contributions.
    pub contributions: Vec<Contribution>,
}

impl HistoryData {
    /// Calculate total tokens used across all contributions.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.contributions
            .iter()
            .filter_map(|c| c.ai_stats.as_ref())
            .map(|stats| stats.input_tokens + stats.output_tokens)
            .sum()
    }

    /// Calculate total cost in USD across all contributions.
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.contributions
            .iter()
            .filter_map(|c| c.ai_stats.as_ref())
            .filter_map(|stats| stats.cost_usd)
            .sum()
    }

    /// Calculate average tokens per triage.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_tokens_per_triage(&self) -> f64 {
        let contributions_with_stats: Vec<_> = self
            .contributions
            .iter()
            .filter_map(|c| c.ai_stats.as_ref())
            .collect();

        if contributions_with_stats.is_empty() {
            return 0.0;
        }

        let total: u64 = contributions_with_stats
            .iter()
            .map(|stats| stats.input_tokens + stats.output_tokens)
            .sum();

        total as f64 / contributions_with_stats.len() as f64
    }

    /// Calculate total cost grouped by model.
    #[must_use]
    pub fn cost_by_model(&self) -> std::collections::HashMap<String, f64> {
        let mut costs = std::collections::HashMap::new();

        for contribution in &self.contributions {
            if let Some(stats) = &contribution.ai_stats
                && let Some(cost) = stats.cost_usd
            {
                *costs.entry(stats.model.clone()).or_insert(0.0) += cost;
            }
        }

        costs
    }
}

/// Returns the path to the history file.
#[must_use]
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
            ai_stats: None,
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

    #[test]
    fn test_ai_stats_serialization_roundtrip() {
        let stats = AiStats {
            provider: "openrouter".to_string(),
            model: "mistralai/mistral-small-2603".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            duration_ms: 1500,
            cost_usd: Some(0.0),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        };

        let json = serde_json::to_string(&stats).expect("serialize");
        let parsed: AiStats = serde_json::from_str(&json).expect("deserialize");

        // After deserialization, ETU is always recomputed from token counts,
        // so we compare all fields except effective_token_units.
        assert_eq!(stats.provider, parsed.provider);
        assert_eq!(stats.model, parsed.model);
        assert_eq!(stats.input_tokens, parsed.input_tokens);
        assert_eq!(stats.output_tokens, parsed.output_tokens);
        assert_eq!(stats.duration_ms, parsed.duration_ms);
        assert_eq!(stats.cost_usd, parsed.cost_usd);
        assert_eq!(stats.fallback_provider, parsed.fallback_provider);
        assert_eq!(stats.prompt_chars, parsed.prompt_chars);
        assert_eq!(stats.cache_read_tokens, parsed.cache_read_tokens);
        assert_eq!(stats.cache_write_tokens, parsed.cache_write_tokens);
        assert_eq!(stats.trace_id, parsed.trace_id);
        // ETU must be recomputed: 1000 input + 500*5 output = 3500.0
        assert!((parsed.effective_token_units - 3500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_contribution_with_ai_stats() {
        let mut contribution = test_contribution();
        contribution.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "mistralai/mistral-small-2603".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            duration_ms: 1500,
            cost_usd: Some(0.0),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let json = serde_json::to_string(&contribution).expect("serialize");
        let parsed: Contribution = serde_json::from_str(&json).expect("deserialize");

        assert!(parsed.ai_stats.is_some());
        assert_eq!(
            parsed.ai_stats.unwrap().model,
            "mistralai/mistral-small-2603"
        );
    }

    #[test]
    fn test_contribution_without_ai_stats_backward_compat() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "repo": "owner/repo",
            "issue": 123,
            "action": "triage",
            "timestamp": "2024-01-01T00:00:00Z",
            "comment_url": "https://github.com/owner/repo/issues/123#issuecomment-1",
            "status": "pending"
        }"#;

        let parsed: Contribution = serde_json::from_str(json).expect("deserialize");
        assert!(parsed.ai_stats.is_none());
    }

    #[test]
    fn test_total_tokens() {
        let mut data = HistoryData::default();

        let mut c1 = test_contribution();
        c1.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model1".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: Some(0.01),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let mut c2 = test_contribution();
        c2.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model2".to_string(),
            input_tokens: 200,
            output_tokens: 100,
            duration_ms: 2000,
            cost_usd: Some(0.02),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        data.contributions.push(c1);
        data.contributions.push(c2);
        data.contributions.push(test_contribution()); // No stats

        assert_eq!(data.total_tokens(), 450);
    }

    #[test]
    fn test_total_cost() {
        let mut data = HistoryData::default();

        let mut c1 = test_contribution();
        c1.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model1".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: Some(0.01),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let mut c2 = test_contribution();
        c2.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model2".to_string(),
            input_tokens: 200,
            output_tokens: 100,
            duration_ms: 2000,
            cost_usd: Some(0.02),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        data.contributions.push(c1);
        data.contributions.push(c2);

        assert!((data.total_cost() - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_tokens_per_triage() {
        let mut data = HistoryData::default();

        let mut c1 = test_contribution();
        c1.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model1".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: Some(0.01),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let mut c2 = test_contribution();
        c2.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model2".to_string(),
            input_tokens: 200,
            output_tokens: 100,
            duration_ms: 2000,
            cost_usd: Some(0.02),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        data.contributions.push(c1);
        data.contributions.push(c2);

        assert!((data.avg_tokens_per_triage() - 225.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_tokens_per_triage_empty() {
        let data = HistoryData::default();
        assert!((data.avg_tokens_per_triage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cost_by_model() {
        let mut data = HistoryData::default();

        let mut c1 = test_contribution();
        c1.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model1".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: Some(0.01),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let mut c2 = test_contribution();
        c2.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model1".to_string(),
            input_tokens: 200,
            output_tokens: 100,
            duration_ms: 2000,
            cost_usd: Some(0.02),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        let mut c3 = test_contribution();
        c3.ai_stats = Some(AiStats {
            provider: "openrouter".to_string(),
            model: "model2".to_string(),
            input_tokens: 150,
            output_tokens: 75,
            duration_ms: 1500,
            cost_usd: Some(0.015),
            fallback_provider: None,
            prompt_chars: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        });

        data.contributions.push(c1);
        data.contributions.push(c2);
        data.contributions.push(c3);

        let costs = data.cost_by_model();
        assert_eq!(costs.len(), 2);
        assert!((costs.get("model1").unwrap() - 0.03).abs() < f64::EPSILON);
        assert!((costs.get("model2").unwrap() - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ai_stats_cache_tokens_roundtrip() {
        let stats = AiStats {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            duration_ms: 1500,
            cost_usd: Some(0.05),
            fallback_provider: None,
            prompt_chars: 5000,
            cache_read_tokens: 100,
            cache_write_tokens: 50,
            effective_token_units: 0.0,
            trace_id: None,
        };

        let json = serde_json::to_string(&stats).expect("serialize");
        let parsed: AiStats = serde_json::from_str(&json).expect("deserialize");

        // After deserialization, ETU is always recomputed from token counts,
        // so we compare all fields except effective_token_units.
        assert_eq!(stats.provider, parsed.provider);
        assert_eq!(stats.model, parsed.model);
        assert_eq!(stats.input_tokens, parsed.input_tokens);
        assert_eq!(stats.output_tokens, parsed.output_tokens);
        assert_eq!(stats.duration_ms, parsed.duration_ms);
        assert_eq!(stats.cost_usd, parsed.cost_usd);
        assert_eq!(stats.fallback_provider, parsed.fallback_provider);
        assert_eq!(stats.prompt_chars, parsed.prompt_chars);
        assert_eq!(stats.cache_read_tokens, 100);
        assert_eq!(stats.cache_write_tokens, 50);
        assert_eq!(parsed.cache_read_tokens, 100);
        assert_eq!(parsed.cache_write_tokens, 50);
        // ETU must be recomputed: 1000 input + 0.1*100 cache_read + 1.25*50 cache_write + 500*5 output = 3572.5
        assert!((parsed.effective_token_units - 3572.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ai_stats_cache_tokens_default() {
        let json = r#"{
            "provider": "openrouter",
            "model": "mistralai/mistral-small-2603",
            "input_tokens": 1000,
            "output_tokens": 500,
            "duration_ms": 1500,
            "cost_usd": 0.0,
            "fallback_provider": null,
            "prompt_chars": 0
        }"#;

        let parsed: AiStats = serde_json::from_str(json).expect("deserialize");

        assert_eq!(parsed.cache_read_tokens, 0);
        assert_eq!(parsed.cache_write_tokens, 0);
    }

    #[test]
    fn test_etu_formula() {
        // All four token classes with non-trivial values.
        // input(1000) + cache_read(0.1*500=50) + cache_write(1.25*100=125) + output(5.0*200=1000) = 2175.0
        let stats = AiStats {
            input_tokens: 1000,
            output_tokens: 200,
            cache_read_tokens: 500,
            cache_write_tokens: 100,
            ..AiStats::default()
        }
        .with_computed_etu();
        assert!((stats.effective_token_units - 2175.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_etu_zero_on_default() {
        // Zero inputs produce zero ETU; also covers the serde default path.
        let stats = AiStats::default().with_computed_etu();
        assert_eq!(stats.effective_token_units, 0.0);
    }

    #[test]
    fn test_etu_recomputed_on_deserialize() {
        // A JSON record with a stale/wrong effective_token_units value.
        // After deserialization the field must be recomputed from token counts.
        let json = r#"{
            "provider": "anthropic",
            "model": "claude-sonnet-4-6",
            "input_tokens": 1000,
            "output_tokens": 200,
            "cache_read_tokens": 500,
            "cache_write_tokens": 100,
            "effective_token_units": 99999.0
        }"#;
        let stats: AiStats = serde_json::from_str(json).unwrap();
        // Must equal compute_etu(1000, 500, 100, 200) = 2175.0, not 99999.0
        assert!((stats.effective_token_units - 2175.0).abs() < f64::EPSILON);
    }
}
