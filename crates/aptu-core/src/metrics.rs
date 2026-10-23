// SPDX-FileCopyrightText: 2026 Aptu Contributors
// SPDX-License-Identifier: Apache-2.0

//! Fire-and-forget JSONL metrics logging.
//!
//! Appends AI usage statistics to a JSONL file when `APTU_METRICS_FILE` environment variable is set.
//! Appends PR review context records to a JSONL file when `APTU_CONTEXT_FILE` environment variable is set.
//! Failures are logged as warnings and never propagate to the caller.

use std::fs::OpenOptions;
use std::io::Write;

use serde::{Deserialize, Serialize};

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

/// AI usage statistics for an AI-backed operation.
///
/// Deserialization uses `serde(default)` on every field (notably
/// `effective_token_units`) so legacy JSONL rows that predate newer
/// fields still load.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AiStats {
    /// Provider name (e.g., "openrouter", "anthropic").
    #[serde(default)]
    pub provider: String,
    /// Model used for analysis.
    #[serde(default)]
    pub model: String,
    /// Number of input tokens.
    #[serde(default)]
    pub input_tokens: u64,
    /// Number of output tokens.
    #[serde(default)]
    pub output_tokens: u64,
    /// Duration of the API call in milliseconds.
    #[serde(default)]
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

/// Append an AI statistics record to the metrics JSONL file.
///
/// Reads the `APTU_METRICS_FILE` environment variable. If not set, this is a no-op.
/// If set, opens the file in append mode (creating it if necessary) and writes a single
/// JSON line followed by a newline.
///
/// On any error (file I/O, serialization), logs a warning and returns normally.
/// This function never fails the caller's operation.
pub fn append_jsonl(stats: &AiStats) {
    let Ok(path) = std::env::var("APTU_METRICS_FILE") else {
        return; // Env var not set; no-op
    };

    if let Err(e) = append_jsonl_impl(&path, stats) {
        tracing::warn!("metrics: failed to append JSONL record: {}", e);
    }
}

fn append_jsonl_impl(path: &str, stats: &AiStats) -> std::io::Result<()> {
    let json_line = serde_json::to_string(stats)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut file = OpenOptions::new().append(true).create(true).open(path)?;

    file.write_all(json_line.as_bytes())?;
    file.write_all(b"\n")?;

    Ok(())
}

/// Record of PR review context decisions for explainability.
///
/// Captures all context assembly decisions (files, enrichments, budget drops, prompt size)
/// for a single PR review operation. Written to JSONL when `APTU_CONTEXT_FILE` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewContextRecord {
    /// Unique trace ID for correlating with AI stats.
    pub trace_id: String,
    /// Operation type (e.g., `pr_review`).
    pub operation: String,
    /// PR identifier (owner/repo#number).
    pub pr: String,
    /// Model used for analysis.
    pub model: String,
    /// GitHub actor (if available from environment).
    pub github_actor: Option<String>,
    /// Total number of files in the PR.
    pub files_total: usize,
    /// Number of files with a patch (non-empty diff).
    pub files_with_patch: usize,
    /// Number of files whose full content was truncated.
    pub files_truncated: usize,
    /// Total characters dropped from truncated files.
    pub truncated_chars_dropped: usize,
    /// Names of files whose patches were truncated or skipped at prompt assembly.
    pub truncated_patch_files: Vec<String>,
    /// Names of context items dropped due to budget (e.g., `full_content`).
    pub budget_drops: Vec<String>,
    /// Final assembled prompt character count.
    pub prompt_chars_final: usize,
    /// Finish reasons from the AI response.
    pub finish_reasons: Vec<String>,
    /// Maximum prompt character budget from review config.
    pub max_prompt_chars: usize,
}

/// Append a PR review context record to the context JSONL file.
///
/// Reads the `APTU_CONTEXT_FILE` environment variable. If not set, this is a no-op.
/// If set, opens the file in append mode (creating it if necessary) and writes a single
/// JSON line followed by a newline.
///
/// On any error (file I/O, serialization), logs a warning and returns normally.
/// This function never fails the caller's operation.
///
/// `APTU_CONTEXT_FILE` is validated to be non-empty when first encountered; an
/// empty value is rejected with a warning so misconfiguration is visible rather
/// than silently dropped.  Open/write errors are also warned and discarded so
/// that a bad path never aborts the caller.
pub fn write_context_jsonl(record: &ReviewContextRecord) {
    let Ok(path) = std::env::var("APTU_CONTEXT_FILE") else {
        return; // Env var not set; no-op
    };

    if path.is_empty() {
        tracing::warn!("metrics: APTU_CONTEXT_FILE is set but empty; skipping context write");
        return;
    }

    if let Err(e) = write_context_jsonl_impl(&path, record) {
        tracing::warn!(
            path = %path,
            error = %e,
            "metrics: failed to write context JSONL record"
        );
    }
}

fn write_context_jsonl_impl(path: &str, record: &ReviewContextRecord) -> std::io::Result<()> {
    let json_line = serde_json::to_string(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut file = OpenOptions::new().append(true).create(true).open(path)?;

    file.write_all(json_line.as_bytes())?;
    file.write_all(b"\n")?;
    if let Err(e) = file.flush() {
        tracing::warn!("aptu: failed to flush context file: {}", e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_append_jsonl_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("metrics.jsonl");
        let file_path_str = file_path.to_string_lossy().into_owned();

        let stats = AiStats {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: Some(0.01),
            fallback_provider: None,
            prompt_chars: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        }
        .with_computed_etu();

        append_jsonl_impl(&file_path_str, &stats).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"provider\":\"test-provider\""));
        assert!(content.contains("\"model\":\"test-model\""));
        assert!(content.contains("\"input_tokens\":100"));
        assert!(content.contains("\"output_tokens\":50"));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_append_jsonl_noop_without_env() {
        // Ensure APTU_METRICS_FILE is not set
        // SAFETY: test-only; single-threaded test environment.
        unsafe {
            std::env::remove_var("APTU_METRICS_FILE");
        }

        let stats = AiStats {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: None,
            fallback_provider: None,
            prompt_chars: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        }
        .with_computed_etu();

        // Should not panic or error
        append_jsonl(&stats);
    }

    #[test]
    fn test_append_jsonl_warn_on_error() {
        // Use an invalid path (directory that doesn't exist)
        // SAFETY: test-only; single-threaded test environment.
        unsafe {
            std::env::set_var("APTU_METRICS_FILE", "/nonexistent/path/metrics.jsonl");
        }

        let stats = AiStats {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            duration_ms: 1000,
            cost_usd: None,
            fallback_provider: None,
            prompt_chars: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            effective_token_units: 0.0,
            trace_id: None,
        }
        .with_computed_etu();

        // Should not panic; logs a warning internally
        append_jsonl(&stats);

        // SAFETY: test-only; single-threaded test environment.
        unsafe {
            std::env::remove_var("APTU_METRICS_FILE");
        }
    }

    #[test]
    fn test_append_jsonl_cache_tokens_in_record() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("metrics.jsonl");
        let file_path_str = file_path.to_string_lossy().into_owned();

        let stats = AiStats {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            input_tokens: 200,
            output_tokens: 75,
            duration_ms: 2000,
            cost_usd: Some(0.02),
            fallback_provider: None,
            prompt_chars: 1000,
            cache_read_tokens: 50,
            cache_write_tokens: 25,
            effective_token_units: 0.0,
            trace_id: None,
        }
        .with_computed_etu();

        append_jsonl_impl(&file_path_str, &stats).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"cache_read_tokens\":50"));
        assert!(content.contains("\"cache_write_tokens\":25"));
    }

    #[test]
    fn test_write_context_jsonl_noop_without_env() {
        // Ensure APTU_CONTEXT_FILE is not set
        // SAFETY: test-only; single-threaded test environment.
        unsafe {
            std::env::remove_var("APTU_CONTEXT_FILE");
        }

        let record = ReviewContextRecord {
            trace_id: "test-trace-id".to_string(),
            operation: "pr_review".to_string(),
            pr: "owner/repo#123".to_string(),
            model: "test-model".to_string(),
            github_actor: None,
            files_total: 5,
            files_with_patch: 4,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            truncated_patch_files: vec![],
            budget_drops: vec![],
            prompt_chars_final: 5000,
            finish_reasons: vec!["stop".to_string()],
            max_prompt_chars: 120_000,
        };

        // Should not panic or error
        write_context_jsonl(&record);
    }

    #[test]
    fn test_write_context_jsonl_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("context.jsonl");
        let file_path_str = file_path.to_string_lossy().into_owned();

        let record = ReviewContextRecord {
            trace_id: "test-trace-id".to_string(),
            operation: "pr_review".to_string(),
            pr: "owner/repo#123".to_string(),
            model: "test-model".to_string(),
            github_actor: Some("test-actor".to_string()),
            files_total: 5,
            files_with_patch: 4,
            files_truncated: 1,
            truncated_chars_dropped: 500,
            truncated_patch_files: vec!["big.rs".to_string()],
            budget_drops: vec!["full_content".to_string()],
            prompt_chars_final: 5000,
            finish_reasons: vec!["stop".to_string()],
            max_prompt_chars: 120_000,
        };

        write_context_jsonl_impl(&file_path_str, &record).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"trace_id\":\"test-trace-id\""));
        assert!(content.contains("\"operation\":\"pr_review\""));
        assert!(content.contains("\"pr\":\"owner/repo#123\""));
        assert!(content.contains("\"files_total\":5"));
        assert!(content.contains("\"files_with_patch\":4"));
        assert!(content.contains("\"github_actor\":\"test-actor\""));
        assert!(content.contains("\"budget_drops\":[\"full_content\"]"));
        assert!(content.contains("\"truncated_patch_files\":[\"big.rs\"]"));
        assert!(content.contains("\"finish_reasons\":[\"stop\"]"));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_max_prompt_chars_recorded_in_context_record() {
        let record = ReviewContextRecord {
            trace_id: "test-trace".to_string(),
            operation: "pr_review".to_string(),
            pr: "owner/repo#1".to_string(),
            model: "gpt-4".to_string(),
            github_actor: None,
            files_total: 5,
            files_with_patch: 3,
            files_truncated: 0,
            truncated_chars_dropped: 0,
            truncated_patch_files: vec![],
            budget_drops: vec![],
            prompt_chars_final: 5000,
            finish_reasons: vec!["stop".to_string()],
            max_prompt_chars: 120_000,
        };

        let json = serde_json::to_string(&record).expect("serialization failed");
        assert!(
            json.contains(r#""max_prompt_chars":120000"#),
            "max_prompt_chars must be present in JSON with correct value, got: {json}",
        );
    }
}
