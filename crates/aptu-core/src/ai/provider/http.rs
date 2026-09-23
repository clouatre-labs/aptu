// SPDX-License-Identifier: Apache-2.0

//! HTTP request sending, retry logic, and response parsing.
//!
//! Provides free-function versions of the trait's HTTP methods:
//! - `send_request_inner`: bare HTTP send with error handling
//! - `try_request`: single HTTP send + JSON parse attempt
//! - `send_and_parse`: retry loop around `try_request` with circuit breaker

use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{debug, instrument};

use super::parse::{parse_ai_json, redact_api_error_body};
use crate::ai::provider::AiProvider;
use crate::ai::registry::consts::PROVIDER_ZAI;
use crate::ai::types::{ChatCompletionRequest, ChatCompletionResponse};
use crate::error::AptuError;
use crate::history::AiStats;
use crate::retry::{extract_retry_after, is_retryable_anyhow};

/// Conservative provider-agnostic ceiling for escalated `max_tokens`.
const MAX_ESCALATED_MAX_TOKENS: u32 = 16384;

fn is_zai_insufficient_balance(error_body: &str) -> bool {
    if error_body
        .to_ascii_lowercase()
        .contains("insufficient balance")
    {
        return true;
    }
    serde_json::from_str::<Value>(error_body)
        .ok()
        .and_then(|v| {
            let code = v
                .get("error")
                .and_then(|e| e.get("code"))
                .or_else(|| v.get("code"));
            code.map(|c| c == "1113" || c.as_i64() == Some(1113))
        })
        .unwrap_or(false)
}

fn map_http_error(
    status: u16,
    provider_name: &str,
    api_key_env: &str,
    retry_after: Option<u64>,
    error_body: &str,
) -> Result<(), AptuError> {
    match status {
        401 => Err(AptuError::AI {
            message: format!(
                "Invalid {provider_name} API key. Check your {api_key_env} environment variable."
            ),
            status: Some(401),
            provider: provider_name.to_string(),
        }),
        429 => {
            let retry_after_val = retry_after.unwrap_or(0);
            debug!(retry_after = retry_after_val, "Parsed Retry-After header");
            if provider_name == PROVIDER_ZAI && is_zai_insufficient_balance(error_body) {
                return Err(AptuError::AI {
                    message: "Z.AI coding plan keys are not valid for the standard API endpoint; \
                        use a pay-as-you-go key or a different fallback provider"
                        .to_string(),
                    status: Some(429),
                    provider: provider_name.to_string(),
                });
            }
            Err(AptuError::RateLimited {
                provider: provider_name.to_string(),
                retry_after: retry_after_val,
            })
        }
        _ => Err(AptuError::AI {
            message: format!(
                "{} API error (HTTP {}): {}",
                provider_name,
                status,
                redact_api_error_body(error_body)
            ),
            status: Some(status),
            provider: provider_name.to_string(),
        }),
    }
}

/// Sends a chat completion request to the provider's API (HTTP-only, no retry).
///
/// Default implementation handles HTTP headers, error responses (401, 429).
/// Does not include retry logic - use `send_and_parse()` for retry behavior.
#[cfg_attr(not(target_arch = "wasm32"), instrument(skip(provider, request), fields(provider = provider.name(), model = provider.model())))]
pub(super) async fn send_request_inner(
    provider: &(impl AiProvider + ?Sized),
    request: &ChatCompletionRequest,
) -> Result<ChatCompletionResponse> {
    use secrecy::ExposeSecret;

    let mut req = provider.http_client().post(provider.api_url());

    // Add Authorization header (skip for Anthropic, which uses x-api-key)
    if !provider.is_anthropic() {
        req = req.header(
            "Authorization",
            format!("Bearer {}", provider.api_key().expose_secret()),
        );
    }

    // Add custom headers from provider
    for (key, value) in &provider.build_headers() {
        req = req.header(key.clone(), value.clone());
    }

    let mut body = serde_json::to_value(request).context(format!(
        "Failed to serialize request for {}",
        provider.name()
    ))?;
    if let Some(extensions) = provider.provider_body_extensions() {
        body["provider"] = extensions;
    }

    let response = req
        .json(&body)
        .send()
        .await
        .context(format!("Failed to send request to {} API", provider.name()))?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        let retry_after = if status.as_u16() == 429 {
            response
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
        } else {
            None
        };
        let error_body = response.text().await.unwrap_or_default();
        return map_http_error(
            status.as_u16(),
            provider.name(),
            provider.api_key_env(),
            retry_after,
            &error_body,
        )
        .map_err(Into::into)
        .map(|()| unreachable!("map_http_error returned Ok for non-success HTTP status"));
    }

    // Parse response
    let completion: ChatCompletionResponse = response.json().await.map_err(|err| {
        if err.is_timeout() {
            anyhow::Error::new(err).context(format!("AI request to {} timed out", provider.name()))
        } else {
            anyhow::Error::new(err)
                .context(format!("Failed to parse {} API response", provider.name()))
        }
    })?;

    Ok(completion)
}

/// Try a single HTTP send + JSON parse.  Separated from `send_and_parse`
/// to avoid closure-in-expression clippy warning.
///
/// `max_tokens_override` replaces the request's `max_tokens` for this attempt,
/// used by `send_and_parse` to escalate the token budget after truncation.
/// A response with `finish_reason == "length"` is treated as truncated and
/// returned as `TruncatedResponse` before JSON parsing (the parse.rs EOF
/// heuristic remains as a fallback for providers that omit `finish_reason`).
#[allow(clippy::items_after_statements)]
pub(super) async fn try_request<T: serde::de::DeserializeOwned>(
    provider: &(impl AiProvider + ?Sized),
    request: &ChatCompletionRequest,
    max_tokens_override: Option<u32>,
) -> Result<(T, ChatCompletionResponse)> {
    // Rebuild the request for this attempt when the token budget is escalated
    let effective_request;
    let request = if max_tokens_override.is_some() && max_tokens_override != request.max_tokens {
        let mut rebuilt = request.clone();
        rebuilt.max_tokens = max_tokens_override;
        effective_request = rebuilt;
        &effective_request
    } else {
        request
    };

    // Send HTTP request
    let completion = send_request_inner(provider, request).await?;

    // Detect explicit provider truncation before attempting to parse the body
    let truncated = completion
        .choices
        .iter()
        .any(|c| c.finish_reason.as_deref() == Some("length"));
    if truncated {
        tracing::warn!(
            provider = provider.name(),
            "Response hit max_tokens limit (finish_reason=length); \
             retrying with an escalated max_tokens budget"
        );
        return Err(anyhow::anyhow!(AptuError::TruncatedResponse {
            provider: provider.name().to_string(),
        }));
    }

    // Extract message content
    let content = completion
        .choices
        .first()
        .and_then(|c| {
            c.message
                .content
                .clone()
                .or_else(|| c.message.reasoning.clone())
        })
        .context("No response from AI model")?;

    debug!(response_length = content.len(), "Received AI response");

    // Parse JSON response (inside retry loop, so truncated responses are retried)
    let parsed: T = parse_ai_json(&content, provider.name())?;

    Ok((parsed, completion))
}

/// Sends a chat completion request and parses the response with retry logic.
///
/// This method wraps the HTTP request in a retry loop (via `try_request`) and retries
/// on transient errors, including truncated JSON responses. Includes circuit breaker
/// handling before the first attempt.
///
/// # Arguments
///
/// * `request` - The chat completion request to send
///
/// # Returns
///
/// A tuple of (parsed response, stats) extracted from the API response
///
/// # Errors
///
/// Returns an error if:
/// - API request fails (network, timeout, rate limit)
/// - Response cannot be parsed as valid JSON (including truncated responses)
#[instrument(skip(provider, request), fields(provider = provider.name(), model = provider.model()))]
#[allow(clippy::too_many_lines)]
pub(super) async fn send_and_parse<T: serde::de::DeserializeOwned + Send>(
    provider: &(impl AiProvider + ?Sized),
    request: &ChatCompletionRequest,
) -> Result<(T, AiStats, Vec<String>)> {
    use tracing::{info, warn};

    // Check circuit breaker before attempting request
    if let Some(cb) = provider.circuit_breaker()
        && cb.is_open()
    {
        return Err(AptuError::CircuitOpen.into());
    }

    // Start timing (outside retry loop to measure total time including retries)
    let start = std::time::Instant::now();

    // Custom retry loop that respects retry_after from RateLimited errors
    let mut attempt: u32 = 0;
    let max_attempts: u32 = provider.max_attempts();

    // Current token budget for the request; escalated (x2) after each
    // truncation retry, capped at MAX_ESCALATED_MAX_TOKENS.
    let mut current_max_tokens: Option<u32> = request.max_tokens;
    // finish_reason values observed across all attempts (telemetry keeps
    // `length` occurrences from failed attempts even after a later success).
    let mut observed_finish_reasons: Vec<String> = Vec::new();

    let (parsed, completion): (T, ChatCompletionResponse) = loop {
        attempt += 1;

        let result = try_request::<T>(provider, request, current_max_tokens).await;

        match result {
            Ok(success) => break success,
            Err(err) => {
                // Check if error is retryable
                if !is_retryable_anyhow(&err) || attempt >= max_attempts {
                    return Err(err);
                }

                // On truncation, escalate the token budget before retrying.
                // A budget already at (or beyond) the cap is not retried:
                // escalating further would only risk provider-side 4xx errors.
                if err
                    .downcast_ref::<AptuError>()
                    .is_some_and(|e| matches!(e, AptuError::TruncatedResponse { .. }))
                {
                    // The truncated attempt observed finish_reason == "length";
                    // keep it for finish_reasons telemetry.
                    observed_finish_reasons.push("length".to_string());
                    match current_max_tokens {
                        Some(mt)
                            if mt >= MAX_ESCALATED_MAX_TOKENS
                                || mt.saturating_mul(2) > MAX_ESCALATED_MAX_TOKENS =>
                        {
                            return Err(anyhow::anyhow!(AptuError::AI {
                                message: format!(
                                    "AI response truncated and max_tokens escalation cap of {MAX_ESCALATED_MAX_TOKENS} reached; \
                                     reduce the review scope or increase the provider's max_tokens budget"
                                ),
                                status: None,
                                provider: provider.name().to_string(),
                            }));
                        }
                        Some(mt) => {
                            let escalated = mt.saturating_mul(2);
                            debug!(
                                previous_max_tokens = mt,
                                escalated_max_tokens = escalated,
                                "Escalating max_tokens after truncated response"
                            );
                            current_max_tokens = Some(escalated);
                        }
                        None => {
                            // No budget to escalate; fall through to the
                            // default backoff retry below.
                        }
                    }
                }

                // Extract retry_after if present, otherwise use exponential backoff
                let delay = if let Some(retry_after_duration) = extract_retry_after(&err) {
                    debug!(
                        retry_after_secs = retry_after_duration.as_secs(),
                        "Using Retry-After value from rate limit error"
                    );
                    retry_after_duration
                } else {
                    // Use exponential backoff with jitter: 1s, 2s, 4s + 0-500ms
                    let backoff_secs = 2_u64.pow(attempt.saturating_sub(1));
                    let jitter_ms = fastrand::u64(0..500);
                    std::time::Duration::from_millis(backoff_secs * 1000 + jitter_ms)
                };

                let error_msg = err.to_string();
                warn!(
                    error = %error_msg,
                    delay_secs = delay.as_secs(),
                    attempt,
                    max_attempts,
                    "Retrying after error"
                );

                // Drop err before await: it is non-Send and must not be held
                // across the sleep boundary. All fields have been extracted above.
                drop(err);
                tokio::time::sleep(delay).await;
            }
        }
    };

    // Record success in circuit breaker
    if let Some(cb) = provider.circuit_breaker() {
        cb.record_success();
    }

    // Calculate duration (total time including any retries)
    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = start.elapsed().as_millis() as u64;

    // Build AI stats from usage info (trust API's cost field)
    let (input_tokens, output_tokens, cost_usd, cache_read_tokens, cache_write_tokens) =
        if let Some(usage) = completion.usage {
            (
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.cost,
                usage.cache_read_tokens,
                usage.cache_write_tokens,
            )
        } else {
            // If no usage info, default to 0
            debug!("No usage information in API response");
            (0, 0, None, 0, 0)
        };

    let ai_stats = AiStats {
        provider: provider.name().to_string(),
        model: provider.model().to_string(),
        input_tokens,
        output_tokens,
        duration_ms,
        cost_usd,
        fallback_provider: None,
        prompt_chars: 0,
        cache_read_tokens,
        cache_write_tokens,
        effective_token_units: 0.0,
        trace_id: None,
    }
    .with_computed_etu();

    // Extract finish_reasons from choices, preserving values observed on
    // failed attempts (e.g. `length`) alongside the successful attempt's.
    let mut finish_reasons: Vec<String> = completion
        .choices
        .iter()
        .filter_map(|c| c.finish_reason.clone())
        .collect();
    finish_reasons.extend(observed_finish_reasons);

    // Emit structured metrics
    info!(
        duration_ms,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost_usd = ?cost_usd,
        model = %provider.model(),
        "AI request completed"
    );

    // Log cache hit/miss details
    debug!(
        cache_read_tokens = %cache_read_tokens,
        cache_write_tokens = %cache_write_tokens,
        "Cache token usage"
    );

    Ok((parsed, ai_stats, finish_reasons))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_error_401() {
        let err = map_http_error(401, "openrouter", "OPENROUTER_API_KEY", None, "").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("openrouter"));
        assert!(msg.contains("OPENROUTER_API_KEY"));
    }

    #[test]
    fn test_map_http_error_429() {
        let err = map_http_error(429, "gemini", "GEMINI_API_KEY", Some(30), "").unwrap_err();
        match err {
            AptuError::RateLimited {
                provider,
                retry_after,
            } => {
                assert_eq!(provider, "gemini");
                assert_eq!(retry_after, 30);
            }
            _ => panic!("expected AptuError::RateLimited, got: {err:?}"),
        }
    }

    #[test]
    fn test_map_http_error_zai_1113_insufficient_balance() {
        let body = r#"{"error":{"code":"1113","message":"Insufficient balance or no resource package. Please recharge."}}"#;
        let err = map_http_error(429, "zai", "ZAI_API_KEY", None, body).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("coding plan keys are not valid"));
        assert!(msg.contains("pay-as-you-go"));
        assert!(!is_retryable_anyhow(&err.into()));
    }

    #[test]
    fn test_map_http_error_zai_1113_numeric_code() {
        let body =
            r#"{"error":{"code":1113,"message":"Insufficient Balance or no resource package."}}"#;
        let err = map_http_error(429, "zai", "ZAI_API_KEY", None, body).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("coding plan keys are not valid"));
        assert!(msg.contains("pay-as-you-go"));
    }

    #[test]
    fn test_map_http_error_zai_1113_top_level_numeric_code() {
        let body = r#"{"code":1113,"message":"insufficient balance"}"#;
        let err = map_http_error(429, "zai", "ZAI_API_KEY", None, body).unwrap_err();
        assert!(err.to_string().contains("coding plan keys are not valid"));
        assert!(!is_retryable_anyhow(&err.into()));
    }

    #[test]
    fn test_map_http_error_zai_other_rate_limit_still_retryable() {
        let body = r#"{"error":{"code":"1001","message":"rate limited"}}"#;
        let err = map_http_error(429, "zai", "ZAI_API_KEY", None, body).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("rate limit"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    struct CircuitOpenProvider {
        breaker: crate::ai::CircuitBreaker,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl AiProvider for CircuitOpenProvider {
        fn config(&self) -> &crate::ai::registry::ProviderConfig {
            &crate::ai::provider::test_utils::TEST_PROVIDER_CONFIG
        }

        fn http_client(&self) -> &reqwest::Client {
            unimplemented!()
        }

        fn api_key(&self) -> &secrecy::SecretString {
            unimplemented!()
        }

        fn circuit_breaker(&self) -> Option<&crate::ai::CircuitBreaker> {
            Some(&self.breaker)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_send_and_parse_circuit_open() {
        let breaker = crate::ai::CircuitBreaker::new(1, 60);
        breaker.record_failure();
        assert!(breaker.is_open());

        let provider = CircuitOpenProvider { breaker };
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
            response_format: None,
            session_id: None,
        };

        let result = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await;

        let err = result.unwrap_err();
        let aptu_err = err
            .downcast_ref::<AptuError>()
            .unwrap_or_else(|| panic!("unexpected error: {err:#}"));
        assert!(matches!(aptu_err, AptuError::CircuitOpen));
    }

    /// Build a minimal HTTP/1.1 200 response with a JSON body for mock servers.
    #[cfg(not(target_arch = "wasm32"))]
    fn mock_http_ok(body: &str) -> String {
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

    /// Build an [`HttpMockProvider`] with the standard test key.
    #[cfg(not(target_arch = "wasm32"))]
    fn mock_provider(
        client: reqwest::Client,
        addr: std::net::SocketAddr,
        max_attempts: u32,
    ) -> HttpMockProvider {
        HttpMockProvider {
            client,
            key: secrecy::SecretString::from("test-key".to_string()),
            url: format!("http://{addr}"),
            max_attempts,
        }
    }

    /// Build a client/provider/request tuple for mock-server tests.
    #[cfg(not(target_arch = "wasm32"))]
    fn mock_setup(
        addr: std::net::SocketAddr,
        max_attempts: u32,
        timeout_ms: Option<u64>,
    ) -> (HttpMockProvider, ChatCompletionRequest) {
        let mut builder = reqwest::Client::builder().pool_max_idle_per_host(0);
        if let Some(ms) = timeout_ms {
            builder = builder.timeout(std::time::Duration::from_millis(ms));
        }
        let client = builder.build().expect("build client");
        let provider = mock_provider(client, addr, max_attempts);
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
            response_format: None,
            session_id: None,
        };
        (provider, request)
    }

    #[cfg(not(target_arch = "wasm32"))]
    struct HttpMockProvider {
        client: reqwest::Client,
        key: secrecy::SecretString,
        url: String,
        max_attempts: u32,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl AiProvider for HttpMockProvider {
        fn config(&self) -> &crate::ai::registry::ProviderConfig {
            &crate::ai::provider::test_utils::TEST_PROVIDER_CONFIG
        }

        fn api_url(&self) -> &str {
            &self.url
        }

        fn http_client(&self) -> &reqwest::Client {
            &self.client
        }

        fn api_key(&self) -> &secrecy::SecretString {
            &self.key
        }

        fn max_attempts(&self) -> u32 {
            self.max_attempts
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_send_and_parse_retry_then_succeed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            // First request: return 429 Rate Limited with Retry-After: 0 and Connection: close
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body = "rate limit exceeded";
                let response = format!(
                    "HTTP/1.1 429 Too Many Requests\r\n\
                     Retry-After: 0\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\
                     \r\n\
                     {}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }

            // Second request: return 200 OK with valid response JSON
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body = r#"{"choices":[{"message":{"role":"assistant","content":"{\"_message\":\"ok\"}"}}]}"#;
                let response = mock_http_ok(body);
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("build client");

        let provider = mock_provider(client, addr, 3);

        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
            response_format: None,
            session_id: None,
        };

        let (parsed, stats, _reasons) = send_and_parse::<
            crate::ai::provider::test_utils::ErrorTestResponse,
        >(&provider, &request)
        .await
        .expect("send_and_parse should succeed after retry");

        assert_eq!(parsed.message, "ok");
        assert_eq!(stats.provider, "test");
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Serves canned 200 responses (body: `finish_reason` + content) and records
    /// the `max_tokens` value of each incoming request body.
    async fn spawn_max_tokens_server(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<u32>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let seen_max_tokens = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = seen_max_tokens.clone();

        tokio::spawn(async move {
            for (finish_reason, content) in responses {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(body_start) = raw.find("\r\n\r\n")
                    && let Ok(body) =
                        serde_json::from_str::<serde_json::Value>(raw[body_start + 4..].trim())
                    && let Some(mt) = body.get("max_tokens").and_then(serde_json::Value::as_u64)
                {
                    seen.lock()
                        .expect("lock")
                        .push(u32::try_from(mt).unwrap_or(u32::MAX));
                }
                let body = serde_json::json!({
                    "choices": [{
                        "message": {"role": "assistant", "content": content},
                        "finish_reason": finish_reason,
                    }]
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        (format!("http://{addr}"), seen_max_tokens)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn truncation_test_request(
        max_tokens: Option<u32>,
        url: String,
        client: reqwest::Client,
        max_attempts: u32,
    ) -> (HttpMockProvider, ChatCompletionRequest) {
        let provider = HttpMockProvider {
            client,
            key: secrecy::SecretString::from("test-key".to_string()),
            url,
            max_attempts,
        };
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            max_tokens,
            temperature: None,
            response_format: None,
            session_id: None,
        };
        (provider, request)
    }

    // Arrange: provider responds with finish_reason "length" even though the
    // JSON body is complete and parseable.
    // Act/Assert: send_and_parse fails with TruncatedResponse before parsing.
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_finish_reason_length_returns_truncated_before_parse() {
        let (url, seen) = spawn_max_tokens_server(vec![("length", "{\"_message\":\"ok\"}")]).await;
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("build client");
        let (provider, request) = truncation_test_request(Some(4096), url, client, 1);

        let err = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await
        .unwrap_err();

        let aptu_err = err
            .downcast_ref::<AptuError>()
            .unwrap_or_else(|| panic!("unexpected error: {err:#}"));
        assert!(matches!(aptu_err, AptuError::TruncatedResponse { .. }));
        assert_eq!(*seen.lock().expect("lock"), vec![4096]);
    }

    // Arrange: first attempt returns finish_reason "length", second succeeds.
    // Act/Assert: the retry sends an escalated (x2) max_tokens and the
    // finish_reasons telemetry records the "length" occurrence.
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_truncation_retry_sends_escalated_max_tokens() {
        let (url, seen) = spawn_max_tokens_server(vec![
            ("length", "{\"_message\":\"partial\"}"),
            ("stop", "{\"_message\":\"ok\"}"),
        ])
        .await;
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("build client");
        let (provider, request) = truncation_test_request(Some(4096), url, client, 2);

        let (parsed, _stats, finish_reasons) = send_and_parse::<
            crate::ai::provider::test_utils::ErrorTestResponse,
        >(&provider, &request)
        .await
        .expect("should succeed after escalated retry");

        assert_eq!(parsed.message, "ok");
        assert_eq!(*seen.lock().expect("lock"), vec![4096, 8192]);
        assert!(finish_reasons.contains(&"length".to_string()));
        assert!(finish_reasons.contains(&"stop".to_string()));
    }

    // Arrange: max_tokens starts at half the escalation cap, so a single
    // doubling would exceed MAX_ESCALATED_MAX_TOKENS.
    // Act/Assert: a distinct non-retryable AI error is returned instead of
    // retrying at (or beyond) the cap.
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_truncation_cap_reached_is_non_retryable() {
        let (url, seen) =
            spawn_max_tokens_server(vec![("length", "{\"_message\":\"partial\"}")]).await;
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("build client");
        let (provider, request) = truncation_test_request(Some(10000), url, client, 3);
        let err = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await
        .unwrap_err();

        let aptu_err = err
            .downcast_ref::<AptuError>()
            .unwrap_or_else(|| panic!("unexpected error: {err:#}"));
        match aptu_err {
            AptuError::AI { message, .. } => assert!(message.contains("cap")),
            other => panic!("expected non-retryable AI error, got: {other:?}"),
        }
        // No second attempt was made after the cap was hit.
        assert_eq!(*seen.lock().expect("lock"), vec![10000]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_send_and_parse_timeout_yields_timeout_context_and_stays_retryable() {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            // Respond with a Content-Length larger than the written body, then
            // stall, forcing a client timeout during the body read (json()).
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body = "hi";
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: 500\r\n\
                     Connection: close\r\n\
                     \r\n\
                     {body}"
                );
                let _ = stream.write_all(response.as_bytes()).await;
                tokio::time::sleep(Duration::from_secs(10)).await;
                let _ = stream.shutdown().await;
            }
        });

        let (provider, request) = mock_setup(addr, 1, Some(300));

        let err = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("AI request to test timed out"),
            "unexpected message: {msg}"
        );
        assert!(!msg.contains("Failed to parse"));
        // Timeout classification must survive the context layer.
        assert!(
            is_retryable_anyhow(&err),
            "timeout error should remain retryable"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_send_and_parse_malformed_json_yields_parse_context() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body = "not json at all";
                let response = mock_http_ok(body);
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        let (provider, request) = mock_setup(addr, 1, None);

        let err = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("Failed to parse test API response"),
            "unexpected message: {msg}"
        );
        assert!(!msg.contains("timed out"));
    }
}
