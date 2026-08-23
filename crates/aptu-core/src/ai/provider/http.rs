// SPDX-License-Identifier: Apache-2.0

//! HTTP request sending, retry logic, and response parsing.
//!
//! Provides free-function versions of the trait's HTTP methods:
//! - `send_request_inner`: bare HTTP send with error handling
//! - `try_request`: single HTTP send + JSON parse attempt
//! - `send_and_parse`: retry loop around `try_request` with circuit breaker

use anyhow::{Context, Result};
use tracing::{debug, instrument};

use super::parse::{parse_ai_json, redact_api_error_body};
use crate::ai::provider::AiProvider;
use crate::ai::types::{ChatCompletionRequest, ChatCompletionResponse};
use crate::error::AptuError;
use crate::history::AiStats;
use crate::retry::{extract_retry_after, is_retryable_anyhow};

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
    let completion: ChatCompletionResponse = response
        .json()
        .await
        .context(format!("Failed to parse {} API response", provider.name()))?;

    Ok(completion)
}

/// Try a single HTTP send + JSON parse.  Separated from `send_and_parse`
/// to avoid closure-in-expression clippy warning.
#[allow(clippy::items_after_statements)]
pub(super) async fn try_request<T: serde::de::DeserializeOwned>(
    provider: &(impl AiProvider + ?Sized),
    request: &ChatCompletionRequest,
) -> Result<(T, ChatCompletionResponse)> {
    // Send HTTP request
    let completion = send_request_inner(provider, request).await?;

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

    let (parsed, completion): (T, ChatCompletionResponse) = loop {
        attempt += 1;

        let result = try_request(provider, request).await;

        match result {
            Ok(success) => break success,
            Err(err) => {
                // Check if error is retryable
                if !is_retryable_anyhow(&err) || attempt >= max_attempts {
                    return Err(err);
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

    // Extract finish_reasons from choices
    let finish_reasons: Vec<String> = completion
        .choices
        .iter()
        .filter_map(|c| c.finish_reason.clone())
        .collect();

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
        };

        let result = send_and_parse::<crate::ai::provider::test_utils::ErrorTestResponse>(
            &provider, &request,
        )
        .await;

        let err = result.unwrap_err();
        let aptu_err = err.downcast_ref::<AptuError>().expect("expected AptuError");
        assert!(matches!(aptu_err, AptuError::CircuitOpen));
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
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
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
        });

        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("build client");

        let provider = HttpMockProvider {
            client,
            key: secrecy::SecretString::from("test-key".to_string()),
            url: format!("http://{addr}"),
            max_attempts: 3,
        };

        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
            response_format: None,
        };

        let (parsed, stats, _reasons) = send_and_parse::<
            crate::ai::provider::test_utils::ErrorTestResponse,
        >(&provider, &request)
        .await
        .expect("send_and_parse should succeed after retry");

        assert_eq!(parsed._message, "ok");
        assert_eq!(stats.provider, "test");
    }
}
