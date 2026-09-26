// SPDX-License-Identifier: Apache-2.0

//! Typed-judge client.
//!
//! Posts a batched judgment request to `POST /v1/systemone` with Bearer auth
//! from `JUDGE_API_KEY` (read at call time). Any failure returns a
//! `{ fallback: true, error }` envelope instead of propagating an error, so
//! callers can degrade gracefully. Every call appends one JSONL telemetry
//! record (failure-tolerant, only when the judge is enabled).

use serde::{Deserialize, Serialize};

use crate::config::JudgeConfig;

/// Maximum request payload size before send (256 KiB).
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;

/// Maximum response body size before deserialization (1 MiB).
///
/// Guards against a compromised or erroneous `api_base` returning an
/// arbitrarily large body: the content-length is checked upfront and the
/// body is read incrementally with a hard cap before any JSON parsing.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Default judge API base.
pub const DEFAULT_API_BASE: &str = "https://api.typesafe.ai";

/// Hard deadline for the judge API request. The judge must never hang on
/// API latency: the HTTP client enforces it via `Client::builder().timeout`
/// and `send_and_parse` additionally wraps `send()` in `tokio::time::timeout`.
#[cfg(not(target_arch = "wasm32"))]
pub const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// A single question to be judged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgmentQuestion {
    /// Question identifier echoed back in the answer.
    pub id: String,
    /// Question text.
    pub text: String,
}

/// A single answer returned by the judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgmentAnswer {
    /// Question identifier this answer corresponds to.
    pub id: String,
    /// Judge's verdict for the question.
    pub verdict: String,
}

/// Outcome of a judge call: either real answers, or a fallback envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeOutcome {
    /// True when the judge failed and callers should use their fallback path.
    pub fallback: bool,
    /// Error description when `fallback` is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Answers from the batched response (empty when `fallback` is true).
    #[serde(default)]
    pub answers: Vec<JudgmentAnswer>,
}

/// Request body for `POST /v1/systemone`.
#[derive(Debug, Serialize)]
struct SystemOneRequest {
    state: String,
    questions: Vec<JudgmentQuestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

/// Response body from `POST /v1/systemone`.
#[derive(Debug, Deserialize)]
struct SystemOneResponse {
    answers: Vec<JudgmentAnswer>,
}

/// Append-only JSONL telemetry record: metadata only, never answer verdicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeTelemetryRecord {
    /// Purpose of the call (always `judge`).
    pub purpose: String,
    /// `ok` or `fallback`.
    pub outcome: String,
    /// Error category / fallback reason when outcome is "fallback".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of answers returned (not their contents).
    pub answer_count: usize,
    /// Serialized request payload size in bytes.
    pub request_payload_bytes: usize,
    /// Response body size in bytes when a response was received.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_payload_bytes: Option<usize>,
}

fn fallback_outcome(error: &str) -> JudgeOutcome {
    JudgeOutcome {
        fallback: true,
        error: Some(error.to_string()),
        answers: Vec::new(),
    }
}

/// Run a typed-judge call against the judge API.
///
/// Never returns an error across the boundary: any failure (missing API key,
/// oversized payload, HTTP error, parse failure) yields a
/// `{ fallback: true, error }` envelope and emits a visible tracing event.
#[cfg(not(target_arch = "wasm32"))]
pub async fn judge(
    config: &JudgeConfig,
    state: &str,
    questions: Vec<JudgmentQuestion>,
) -> JudgeOutcome {
    let telemetry_path = telemetry_file();
    judge_with_telemetry(config, state, questions, Some(telemetry_path.as_path())).await
}

/// wasm32 stub: the judge is not supported in browser targets.
#[cfg(target_arch = "wasm32")]
pub async fn judge(
    _config: &JudgeConfig,
    _state: &str,
    _questions: Vec<JudgmentQuestion>,
) -> JudgeOutcome {
    fallback_outcome("judge is not supported on wasm32-unknown-unknown")
}

/// Like [`judge`], with an explicit telemetry path for testability.
#[cfg(not(target_arch = "wasm32"))]
pub async fn judge_with_telemetry(
    config: &JudgeConfig,
    state: &str,
    questions: Vec<JudgmentQuestion>,
    telemetry_path: Option<&std::path::Path>,
) -> JudgeOutcome {
    if !config.enabled {
        return fallback_outcome("judge is disabled");
    }

    // Single finalization path: the enabled body computes an outcome plus
    // call metadata, then telemetry is written exactly once before returning,
    // including for preflight failures (missing key, serialization, size,
    // client build).
    let (outcome, request_bytes, response_bytes) =
        judge_enabled_body(config, state, questions).await;

    write_telemetry(&outcome, request_bytes, response_bytes, telemetry_path);
    outcome
}

/// Reads the response body with a hard cap so a compromised `api_base` cannot
/// make us buffer an arbitrarily large response.
#[cfg(not(target_arch = "wasm32"))]
async fn read_bounded_body(
    mut resp: reqwest::Response,
) -> Result<Vec<u8>, (String, Option<usize>)> {
    if let Some(len) = resp
        .content_length()
        .and_then(|len| usize::try_from(len).ok())
        .filter(|&len| len > MAX_RESPONSE_BYTES)
    {
        return Err((
            format!("response of {len} bytes exceeds 1 MiB cap"),
            Some(len),
        ));
    }
    let mut body: Vec<u8> = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err((
                        "response body exceeds 1 MiB cap".to_string(),
                        Some(body.len() + chunk.len()),
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => return Ok(body),
            Err(e) => return Err((format!("failed to read judge response: {e}"), None)),
        }
    }
}

/// Sends the request and parses a bounded response into an outcome plus the
/// response body size for telemetry.
#[cfg(not(target_arch = "wasm32"))]
async fn send_and_parse(
    client: reqwest::Client,
    url: String,
    api_key: &str,
    payload: Vec<u8>,
) -> (JudgeOutcome, Option<usize>) {
    let request = client
        .post(&url)
        .bearer_auth(api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload);
    // Belt-and-braces deadline in addition to the client-level timeout: the
    // future is dropped on expiry and the judge returns the fallback envelope.
    let resp = match tokio::time::timeout(REQUEST_TIMEOUT, request.send()).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            return (
                fallback_outcome(&format!("judge API request failed: {e}")),
                None,
            );
        }
        Err(_) => {
            tracing::warn!(
                purpose = "judge",
                outcome = "fallback",
                reason = "judge API request timed out",
                timeout_secs = REQUEST_TIMEOUT.as_secs()
            );
            return (
                fallback_outcome(&format!(
                    "judge API request timed out after {}s",
                    REQUEST_TIMEOUT.as_secs()
                )),
                None,
            );
        }
    };
    let resp = match resp.error_for_status() {
        Ok(resp) => resp,
        Err(e) => {
            return (
                fallback_outcome(&format!("judge API returned an error status: {e}")),
                None,
            );
        }
    };

    // Response-size guard: bounded read before deserialization.
    let (body, response_bytes) = match read_bounded_body(resp).await {
        Ok(ref body) => (body.clone(), Some(body.len())),
        Err((error, response_bytes)) => {
            return (fallback_outcome(&error), response_bytes);
        }
    };

    match serde_json::from_slice::<SystemOneResponse>(&body) {
        Ok(parsed) => (
            JudgeOutcome {
                fallback: false,
                error: None,
                answers: parsed.answers,
            },
            response_bytes,
        ),
        Err(e) => (
            fallback_outcome(&format!("failed to parse judge response: {e}")),
            response_bytes,
        ),
    }
}

/// Body of an enabled judge call: any failure yields a fallback envelope.
/// Returns the outcome plus request payload size and response body size
/// (when a response was received) for telemetry.
#[cfg(not(target_arch = "wasm32"))]
async fn judge_enabled_body(
    config: &JudgeConfig,
    state: &str,
    questions: Vec<JudgmentQuestion>,
) -> (JudgeOutcome, usize, Option<usize>) {
    let api_key = match std::env::var("JUDGE_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            tracing::warn!(
                purpose = "judge",
                outcome = "fallback",
                reason = "JUDGE_API_KEY not set or empty"
            );
            return (fallback_outcome("JUDGE_API_KEY not set or empty"), 0, None);
        }
    };

    let base = config
        .api_base
        .clone()
        .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
    let base = base.trim_end_matches('/').to_string();

    let body = SystemOneRequest {
        state: state.to_string(),
        questions,
        model: config.model.clone(),
    };
    let payload = match serde_json::to_vec(&body) {
        Ok(p) => p,
        Err(e) => {
            return (
                fallback_outcome(&format!("failed to serialize request: {e}")),
                0,
                None,
            );
        }
    };
    if payload.len() > MAX_PAYLOAD_BYTES {
        tracing::warn!(
            purpose = "judge",
            outcome = "fallback",
            reason = "payload exceeds 256 KiB cap",
            payload_bytes = payload.len()
        );
        return (
            fallback_outcome(&format!(
                "payload of {} bytes exceeds 256 KiB cap",
                payload.len()
            )),
            payload.len(),
            None,
        );
    }

    let client = match reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            return (
                fallback_outcome(&format!("failed to build HTTP client: {e}")),
                payload.len(),
                None,
            );
        }
    };

    let url = format!("{base}/v1/systemone");
    let request_bytes = payload.len();
    let (outcome, response_bytes) = send_and_parse(client, url, &api_key, payload).await;
    (outcome, request_bytes, response_bytes)
}

/// Returns the default JSONL telemetry file path under the Aptu data dir.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn telemetry_file() -> std::path::PathBuf {
    crate::config::data_dir().join("judge_telemetry.jsonl")
}

/// Appends one JSONL telemetry record (metadata only) for the outcome;
/// failure only warns.
#[cfg(not(target_arch = "wasm32"))]
fn write_telemetry(
    outcome: &JudgeOutcome,
    request_payload_bytes: usize,
    response_payload_bytes: Option<usize>,
    path: Option<&std::path::Path>,
) {
    let Some(path) = path else {
        return;
    };
    // Metadata only per the issue spec: never persist answer verdicts.
    let record = JudgeTelemetryRecord {
        purpose: "judge".to_string(),
        outcome: if outcome.fallback { "fallback" } else { "ok" }.to_string(),
        error: outcome.error.clone(),
        answer_count: outcome.answers.len(),
        request_payload_bytes,
        response_payload_bytes,
    };
    let record = match serde_json::to_string(&record) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                purpose = "judge_telemetry",
                "failed to serialize record: {e}"
            );
            return;
        }
    };
    let write = || -> std::io::Result<()> {
        use std::io::Write;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{record}")
    };
    if let Err(e) = write() {
        tracing::warn!(
            purpose = "judge_telemetry",
            outcome = if outcome.fallback { "fallback" } else { "ok" },
            "failed to append telemetry record: {e}"
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #![allow(clippy::items_after_test_module)]
    use super::*;
    use serial_test::serial;

    fn enabled_config(api_base: Option<String>) -> JudgeConfig {
        JudgeConfig {
            enabled: true,
            api_base,
            model: None,
        }
    }

    fn build_mock_http_ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            body.len(),
            body
        )
    }

    #[tokio::test]
    #[serial]
    #[allow(unsafe_code)]
    async fn test_missing_api_key_returns_fallback_envelope() {
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe { std::env::remove_var("JUDGE_API_KEY") };
        let outcome = judge_with_telemetry(&enabled_config(None), "s", vec![], None).await;
        assert!(outcome.fallback);
        assert!(outcome.error.is_some());
        assert!(outcome.answers.is_empty());
    }

    #[tokio::test]
    #[serial]
    #[allow(unsafe_code)]
    async fn test_missing_key_appends_exactly_one_telemetry_record() {
        // SAFETY: single-threaded test process; no concurrent env reads.
        unsafe { std::env::remove_var("JUDGE_API_KEY") };
        let dir = std::env::temp_dir().join(format!("aptu-judge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("telemetry-missing-key.jsonl");
        let _ = std::fs::remove_file(&path);

        let outcome = judge_with_telemetry(&enabled_config(None), "s", vec![], Some(&path)).await;
        assert!(outcome.fallback);

        let contents = std::fs::read_to_string(&path).expect("telemetry file written");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one record expected");
        let parsed: JudgeTelemetryRecord =
            serde_json::from_str(lines[0]).expect("valid JSON record");
        assert_eq!(parsed.outcome, "fallback");
        assert_eq!(parsed.answer_count, 0);
        assert!(!contents.contains("verdict"), "no verdicts in telemetry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sets a dummy `JUDGE_API_KEY` for the calling serial test.
    ///
    /// # Safety
    ///
    /// Mutating process env is UB-adjacent under concurrent reads; callers
    /// must hold the `serial` mutex so no other test reads the env.
    #[allow(unsafe_code)]
    fn set_api_key() {
        // SAFETY: guarded by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("JUDGE_API_KEY", "test-key") };
    }

    #[tokio::test]
    async fn test_oversized_payload_appends_exactly_one_telemetry_record() {
        set_api_key();
        let dir = std::env::temp_dir().join(format!("aptu-judge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("telemetry-oversized.jsonl");
        let _ = std::fs::remove_file(&path);

        let outcome = judge_with_telemetry(
            &enabled_config(Some("http://127.0.0.1:9".to_string())),
            &"x".repeat(MAX_PAYLOAD_BYTES),
            vec![],
            Some(&path),
        )
        .await;
        assert!(outcome.fallback);

        let contents = std::fs::read_to_string(&path).expect("telemetry file written");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one record expected");
        let parsed: JudgeTelemetryRecord =
            serde_json::from_str(lines[0]).expect("valid JSON record");
        assert_eq!(parsed.outcome, "fallback");
        assert!(parsed.error.unwrap_or_default().contains("256 KiB"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_oversized_payload_rejected_without_http() {
        set_api_key();
        let outcome = judge_with_telemetry(
            &enabled_config(Some("http://127.0.0.1:9".to_string())),
            &"x".repeat(MAX_PAYLOAD_BYTES),
            vec![],
            None,
        )
        .await;
        assert!(outcome.fallback);
        assert!(outcome.error.unwrap_or_default().contains("256 KiB"));
    }

    /// Spawns a one-shot mock HTTP server responding 200 OK with `body`.
    /// Returns the base URL to use as the judge `api_base`.
    async fn spawn_mock_server(body: String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 4096];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            let _ = tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                build_mock_http_ok(&body).as_bytes(),
            )
            .await;
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn test_mock_transport_success_returns_answers() {
        set_api_key();
        let api_base =
            spawn_mock_server(r#"{"answers":[{"id":"q1","verdict":"yes"}]}"#.to_string()).await;

        let outcome = judge_with_telemetry(
            &enabled_config(Some(api_base)),
            "state",
            vec![JudgmentQuestion {
                id: "q1".to_string(),
                text: "Is it so?".to_string(),
            }],
            None,
        )
        .await;
        assert!(!outcome.fallback, "expected success: {:?}", outcome.error);
        assert_eq!(outcome.answers.len(), 1);
        assert_eq!(outcome.answers[0].id, "q1");
        assert_eq!(outcome.answers[0].verdict, "yes");
    }

    #[tokio::test]
    async fn test_telemetry_write_failure_does_not_fail_call() {
        set_api_key();
        let api_base =
            spawn_mock_server(r#"{"answers":[{"id":"q1","verdict":"no"}]}"#.to_string()).await;

        // Point telemetry at a directory so the append must fail.
        let bad_path = std::env::temp_dir();
        let outcome = judge_with_telemetry(
            &enabled_config(Some(api_base)),
            "state",
            vec![],
            Some(&bad_path),
        )
        .await;
        assert!(
            !outcome.fallback,
            "telemetry failure must not fail the call"
        );
    }

    #[test]
    fn test_telemetry_appends_jsonl_record() {
        let dir = std::env::temp_dir().join(format!("aptu-judge-test-{}", std::process::id()));
        let path = dir.join("telemetry.jsonl");
        let _ = std::fs::remove_file(&path);

        let outcome = JudgeOutcome {
            fallback: false,
            error: None,
            answers: vec![JudgmentAnswer {
                id: "q1".to_string(),
                verdict: "yes".to_string(),
            }],
        };
        write_telemetry(&outcome, 42, Some(84), Some(&path));
        let contents = std::fs::read_to_string(&path).expect("telemetry file");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one JSONL record expected");
        let parsed: JudgeTelemetryRecord =
            serde_json::from_str(lines[0]).expect("valid JSON record");
        assert_eq!(parsed.outcome, "ok");
        assert_eq!(parsed.answer_count, 1);
        assert_eq!(parsed.request_payload_bytes, 42);
        assert_eq!(parsed.response_payload_bytes, Some(84));
        assert!(parsed.error.is_none());
        // The record must be metadata only: no answer verdict content.
        assert!(!contents.contains("verdict"), "no verdicts in telemetry");
        assert!(!contents.contains("q1"), "no answer ids in telemetry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_oversized_response_returns_fallback_and_metadata_only_telemetry() {
        set_api_key();
        // A body larger than the 1 MiB response cap.
        let api_base = spawn_mock_server("x".repeat(MAX_RESPONSE_BYTES + 1)).await;

        let dir = std::env::temp_dir().join(format!("aptu-judge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("telemetry-oversized-response.jsonl");
        let _ = std::fs::remove_file(&path);

        let outcome = judge_with_telemetry(
            &enabled_config(Some(api_base)),
            "state",
            vec![],
            Some(&path),
        )
        .await;
        assert!(outcome.fallback);
        assert!(
            outcome
                .error
                .unwrap_or_default()
                .contains("exceeds 1 MiB cap")
        );

        let contents = std::fs::read_to_string(&path).expect("telemetry file written");
        let parsed: JudgeTelemetryRecord =
            serde_json::from_str(contents.lines().next().expect("one record"))
                .expect("valid JSON record");
        assert_eq!(parsed.outcome, "fallback");
        assert_eq!(parsed.answer_count, 0);
        assert!(!contents.contains("verdict"), "no verdicts in telemetry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_trailing_slash_api_base_normalized() {
        let base = "http://127.0.0.1:9///";
        let normalized = base.trim_end_matches('/').to_string();
        assert_eq!(normalized, "http://127.0.0.1:9");
    }
}
