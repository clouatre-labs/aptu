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

/// Execute fallback chain when primary provider fails with non-retryable error.
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
                return Err(AptuError::AI {
                    message: e.to_string(),
                    status: None,
                    provider: primary_provider.to_string(),
                });
            }
            warn!(
                primary_provider = primary_provider,
                error = %e,
                "Primary provider failed with non-retryable error, trying fallback chain"
            );
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
