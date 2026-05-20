// SPDX-FileCopyrightText: 2026 Aptu Contributors
// SPDX-License-Identifier: Apache-2.0

//! Fire-and-forget JSONL metrics logging.
//!
//! Appends AI usage statistics to a JSONL file when `APTU_METRICS_FILE` environment variable is set.
//! Appends PR review context records to a JSONL file when `APTU_CONTEXT_FILE` environment variable is set.
//! Failures are logged as warnings and never propagate to the caller.

use std::fs::OpenOptions;
use std::io::Write;

use crate::history::AiStats;
use serde::{Deserialize, Serialize};

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
    /// Characters in AST context.
    pub ast_context_chars: usize,
    /// Characters in call graph context.
    pub call_graph_chars: usize,
    /// Number of dependency enrichments applied.
    pub dep_enrichments_count: usize,
    /// Total characters in dependency enrichments.
    pub dep_enrichments_chars: usize,
    /// Names of context items dropped due to budget (e.g., `call_graph`, `full_content`).
    pub budget_drops: Vec<String>,
    /// Whether the repository path was inferred from CWD.
    pub cwd_inferred: bool,
    /// Final assembled prompt character count.
    pub prompt_chars_final: usize,
    /// Finish reasons from the AI response.
    pub finish_reasons: Vec<String>,
}

/// Append a PR review context record to the context JSONL file.
///
/// Reads the `APTU_CONTEXT_FILE` environment variable. If not set, this is a no-op.
/// If set, opens the file in append mode (creating it if necessary) and writes a single
/// JSON line followed by a newline.
///
/// On any error (file I/O, serialization), logs a warning and returns normally.
/// This function never fails the caller's operation.
pub fn write_context_jsonl(record: &ReviewContextRecord) {
    let Ok(path) = std::env::var("APTU_CONTEXT_FILE") else {
        return; // Env var not set; no-op
    };

    if let Err(e) = write_context_jsonl_impl(&path, record) {
        tracing::warn!("metrics: failed to write context JSONL record: {}", e);
    }
}

fn write_context_jsonl_impl(path: &str, record: &ReviewContextRecord) -> std::io::Result<()> {
    let json_line = serde_json::to_string(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut file = OpenOptions::new().append(true).create(true).open(path)?;

    file.write_all(json_line.as_bytes())?;
    file.write_all(b"\n")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_append_jsonl_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("metrics.jsonl");
        let file_path_str = file_path.to_string_lossy().to_string();

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
            trace_id: None,
        };

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
            trace_id: None,
        };

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
            trace_id: None,
        };

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
        let file_path_str = file_path.to_string_lossy().to_string();

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
            trace_id: None,
        };

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
            ast_context_chars: 1000,
            call_graph_chars: 2000,
            dep_enrichments_count: 2,
            dep_enrichments_chars: 500,
            budget_drops: vec![],
            cwd_inferred: false,
            prompt_chars_final: 5000,
            finish_reasons: vec!["stop".to_string()],
        };

        // Should not panic or error
        write_context_jsonl(&record);
    }

    #[test]
    fn test_write_context_jsonl_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("context.jsonl");
        let file_path_str = file_path.to_string_lossy().to_string();

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
            ast_context_chars: 1000,
            call_graph_chars: 2000,
            dep_enrichments_count: 2,
            dep_enrichments_chars: 500,
            budget_drops: vec!["call_graph".to_string()],
            cwd_inferred: true,
            prompt_chars_final: 5000,
            finish_reasons: vec!["stop".to_string()],
        };

        write_context_jsonl_impl(&file_path_str, &record).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"trace_id\":\"test-trace-id\""));
        assert!(content.contains("\"operation\":\"pr_review\""));
        assert!(content.contains("\"pr\":\"owner/repo#123\""));
        assert!(content.contains("\"files_total\":5"));
        assert!(content.contains("\"files_with_patch\":4"));
        assert!(content.contains("\"github_actor\":\"test-actor\""));
        assert!(content.contains("\"budget_drops\":[\"call_graph\"]"));
        assert!(content.contains("\"finish_reasons\":[\"stop\"]"));
        assert!(content.ends_with('\n'));
    }
}
