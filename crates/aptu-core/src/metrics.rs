// SPDX-FileCopyrightText: 2026 Aptu Contributors
// SPDX-License-Identifier: Apache-2.0

//! Fire-and-forget JSONL metrics logging.
//!
//! Appends AI usage statistics to a JSONL file when `APTU_METRICS_FILE` environment variable is set.
//! Failures are logged as warnings and never propagate to the caller.

use std::fs::OpenOptions;
use std::io::Write;

use crate::history::AiStats;

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
        };

        append_jsonl_impl(&file_path_str, &stats).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\"cache_read_tokens\":50"));
        assert!(content.contains("\"cache_write_tokens\":25"));
    }
}
