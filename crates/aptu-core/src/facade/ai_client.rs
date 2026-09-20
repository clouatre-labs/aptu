// SPDX-License-Identifier: Apache-2.0

//! AI client construction and fallback chain helpers.

use tracing::{info, instrument, warn};

use crate::ai::AiClient;
use crate::ai::registry::get_provider;
use crate::auth::TokenProvider;
use crate::config::AiConfig;
use crate::error::AptuError;
use crate::retry::is_retryable_anyhow;

fn validate_provider_model(provider: &str, model: &str) -> crate::Result<()> {
    // Simple static validation: check if provider exists
    if crate::ai::registry::get_provider(provider).is_none() {
        return Err(AptuError::ModelRegistry {
            message: format!("Provider not found: {provider}"),
        });
    }

    // For now, we allow any model ID (permissive fallback)
    // Unknown models will log a warning but won't fail validation
    tracing::debug!(provider = provider, model = model, "Validating model");
    Ok(())
}

/// Setup and validate primary AI provider synchronously.
/// Returns the created AI client or an error.
fn try_setup_primary_client(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
) -> crate::Result<AiClient> {
    // For Anthropic, delegate to centralized credential resolution
    if primary_provider == "anthropic"
        && let Some(client) = crate::ai::resolve_anthropic_credential(ai_config)
    {
        if ai_config.validation_enabled {
            validate_provider_model(primary_provider, model_name)?;
        }
        return Ok(client);
    }

    // Fall back to environment variable for non-Anthropic or missing Anthropic credentials
    let api_key = provider.ai_api_key(primary_provider).ok_or_else(|| {
        let env_var = get_provider(primary_provider).map_or("API_KEY", |p| p.api_key_env);
        AptuError::AiProviderNotAuthenticated {
            provider: primary_provider.to_string(),
            env_var: env_var.to_string(),
        }
    })?;

    if ai_config.validation_enabled {
        validate_provider_model(primary_provider, model_name)?;
    }

    AiClient::with_api_key(primary_provider, api_key, model_name, ai_config).map_err(|e| {
        AptuError::AI {
            message: e.to_string(),
            status: None,
            provider: primary_provider.to_string(),
        }
    })
}

/// Set up an AI client for a single fallback provider entry.
///
/// Returns `Some(client)` on success, `None` if the entry should be skipped.
fn setup_fallback_client(
    provider: &dyn TokenProvider,
    entry: &crate::config::FallbackEntry,
    model_name: &str,
    ai_config: &AiConfig,
) -> Option<AiClient> {
    let Some(api_key) = provider.ai_api_key(&entry.provider) else {
        warn!(
            fallback_provider = entry.provider,
            "No API key available for fallback provider"
        );
        return None;
    };

    let fallback_model = entry.model.as_deref().unwrap_or(model_name);

    if ai_config.validation_enabled
        && validate_provider_model(&entry.provider, fallback_model).is_err()
    {
        warn!(
            fallback_provider = entry.provider,
            fallback_model = fallback_model,
            "Fallback provider model validation failed, continuing to next provider"
        );
        return None;
    }

    if let Ok(client) = AiClient::with_api_key(&entry.provider, api_key, fallback_model, ai_config)
    {
        Some(client)
    } else {
        warn!(
            fallback_provider = entry.provider,
            "Failed to create AI client for fallback provider"
        );
        None
    }
}

/// Try a single fallback provider entry.
async fn try_fallback_entry<T, F, Fut>(
    provider: &dyn TokenProvider,
    entry: &crate::config::FallbackEntry,
    model_name: &str,
    ai_config: &AiConfig,
    operation: &F,
) -> crate::Result<Option<T>>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    warn!(
        fallback_provider = entry.provider,
        "Attempting fallback provider"
    );

    let Some(ai_client) = setup_fallback_client(provider, entry, model_name, ai_config) else {
        return Ok(None);
    };

    match operation(ai_client).await {
        Ok(response) => {
            info!(
                fallback_provider = entry.provider,
                "Successfully completed operation with fallback provider"
            );
            Ok(Some(response))
        }
        Err(e) => {
            if is_retryable_anyhow(&e) {
                return Err(AptuError::AI {
                    message: e.to_string(),
                    status: None,
                    provider: entry.provider.clone(),
                });
            }
            warn!(
                fallback_provider = entry.provider,
                error = %e,
                "Fallback provider failed with non-retryable error"
            );
            Ok(None)
        }
    }
}

/// Execute the fallback chain when the primary provider fails with a
/// non-retryable error or after rate-limit retries are exhausted.
async fn execute_fallback_chain<T, F, Fut>(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    operation: F,
) -> crate::Result<T>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    if let Some(fallback_config) = &ai_config.fallback {
        for entry in &fallback_config.chain {
            if let Some(response) =
                try_fallback_entry(provider, entry, model_name, ai_config, &operation).await?
            {
                return Ok(response);
            }
        }
    }

    Err(AptuError::AI {
        message: "All AI providers failed (primary and fallback chain)".to_string(),
        status: None,
        provider: primary_provider.to_string(),
    })
}

#[instrument(skip(provider, operation))]
pub(super) async fn try_with_fallback<T, F, Fut>(
    provider: &dyn TokenProvider,
    primary_provider: &str,
    model_name: &str,
    ai_config: &AiConfig,
    operation: F,
) -> crate::Result<T>
where
    F: Fn(AiClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let ai_client = try_setup_primary_client(provider, primary_provider, model_name, ai_config)?;

    match operation(ai_client).await {
        Ok(response) => return Ok(response),
        Err(e) => {
            if is_retryable_anyhow(&e) {
                // RateLimited falls through to the fallback chain after in-loop
                // retry exhaustion; other retryable errors return early.
                if let Some(AptuError::RateLimited { .. }) = e.downcast_ref::<AptuError>() {
                    let chain_configured = ai_config
                        .fallback
                        .as_ref()
                        .is_some_and(|f| !f.chain.is_empty());
                    if !chain_configured {
                        // Return the original error via downcast so its anyhow
                        // context and exact instance are preserved.
                        return match e.downcast::<AptuError>() {
                            Ok(err) => Err(err),
                            Err(e) => Err(AptuError::AI {
                                message: e.to_string(),
                                status: None,
                                provider: primary_provider.to_string(),
                            }),
                        };
                    }
                    info!(
                        primary_provider = primary_provider,
                        "Primary provider rate limited after retry exhaustion, trying fallback chain"
                    );
                } else {
                    return Err(AptuError::AI {
                        message: e.to_string(),
                        status: None,
                        provider: primary_provider.to_string(),
                    });
                }
            } else {
                warn!(
                    primary_provider = primary_provider,
                    error = %e,
                    "Primary provider failed with non-retryable error, trying fallback chain"
                );
            }
        }
    }

    execute_fallback_chain(provider, primary_provider, model_name, ai_config, operation).await
}

#[cfg(test)]
mod tests {
    use crate::config::{FallbackConfig, FallbackEntry};

    #[test]
    fn test_fallback_chain_config_structure() {
        // Test that fallback chain config structure is correct
        let fallback_config = FallbackConfig {
            chain: vec![
                FallbackEntry {
                    provider: "openrouter".to_string(),
                    model: None,
                },
                FallbackEntry {
                    provider: "anthropic".to_string(),
                    model: Some("claude-haiku-4.5".to_string()),
                },
            ],
        };

        assert_eq!(fallback_config.chain.len(), 2);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
        assert_eq!(fallback_config.chain[0].model, None);
        assert_eq!(fallback_config.chain[1].provider, "anthropic");
        assert_eq!(
            fallback_config.chain[1].model,
            Some("claude-haiku-4.5".to_string())
        );
    }

    #[test]
    fn test_fallback_chain_empty() {
        // Test that empty fallback chain is valid
        let fallback_config = FallbackConfig { chain: vec![] };

        assert_eq!(fallback_config.chain.len(), 0);
    }

    #[test]
    fn test_fallback_chain_single_provider() {
        // Test that single provider fallback chain is valid
        let fallback_config = FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "openrouter".to_string(),
                model: None,
            }],
        };

        assert_eq!(fallback_config.chain.len(), 1);
        assert_eq!(fallback_config.chain[0].provider, "openrouter");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod fallback_tests {
    use super::*;
    use crate::auth::TokenProvider;
    use crate::config::{FallbackConfig, FallbackEntry};
    use secrecy::SecretString;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    struct MockTokenProvider;

    impl TokenProvider for MockTokenProvider {
        fn github_token(&self) -> Option<SecretString> {
            None
        }

        fn ai_api_key(&self, provider: &str) -> Option<SecretString> {
            Some(SecretString::from(format!("key-{provider}")))
        }
    }

    fn test_config(fallback: Option<FallbackConfig>) -> AiConfig {
        AiConfig {
            validation_enabled: false,
            fallback,
            ..AiConfig::default()
        }
    }

    /// Arrange: primary call rate limited, fallback call succeeds.
    /// Act: run try_with_fallback.
    /// Assert: the fallback entry executes and the operation succeeds.
    #[tokio::test]
    async fn test_rate_limited_primary_uses_fallback_entry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(anyhow::anyhow!(AptuError::RateLimited {
                        provider: "openrouter".to_string(),
                        retry_after: 1,
                    })) as anyhow::Result<u32>
                } else {
                    let _ = client;
                    Ok(n as u32)
                }
            }
        };

        let config = test_config(Some(FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "groq".to_string(),
                model: None,
            }],
        }));

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            operation,
        )
        .await;

        assert!(result.is_ok(), "expected fallback entry to succeed");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "fallback should be called");
    }

    /// Arrange: primary rate limited with no fallback chain configured.
    /// Act: run try_with_fallback.
    /// Assert: the original RateLimited error is surfaced via downcast,
    /// preserving the original error instance (not a reconstruction).
    #[tokio::test]
    async fn test_rate_limited_without_chain_surfaces_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |_client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!(AptuError::RateLimited {
                    provider: "openrouter".to_string(),
                    retry_after: 7,
                })) as anyhow::Result<u32>
            }
        };

        let config = test_config(None);

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            operation,
        )
        .await;

        match result {
            Err(AptuError::RateLimited {
                provider,
                retry_after,
            }) => {
                assert_eq!(provider, "openrouter");
                assert_eq!(retry_after, 7);
            }
            other => panic!("expected RateLimited error, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no fallback attempted");
    }

    /// Arrange: primary returns TruncatedResponse with a fallback chain configured.
    /// Act: run try_with_fallback.
    /// Assert: returns early without consulting the fallback chain.
    #[tokio::test]
    async fn test_truncated_response_returns_early() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_op = calls.clone();
        let operation = |_client: AiClient| {
            let counter = calls_for_op.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!(AptuError::TruncatedResponse {
                    provider: "openrouter".to_string(),
                })) as anyhow::Result<u32>
            }
        };

        let config = test_config(Some(FallbackConfig {
            chain: vec![FallbackEntry {
                provider: "groq".to_string(),
                model: None,
            }],
        }));

        let result = try_with_fallback(
            &MockTokenProvider,
            "openrouter",
            "test-model",
            &config,
            operation,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "fallback must not run");
    }
}
