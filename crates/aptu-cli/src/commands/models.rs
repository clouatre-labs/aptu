// SPDX-License-Identifier: Apache-2.0

//! Models command handler for listing AI models from providers.

use crate::provider::CliTokenProvider;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Result of listing models from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResult {
    /// Provider name
    pub provider: String,
    /// List of models
    pub models: Vec<SerializableModelInfo>,
}

/// Result of listing models from multiple providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResultMulti {
    /// List of results from each provider
    pub results: Vec<ModelsResult>,
}

/// Serializable model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableModelInfo {
    /// Model identifier
    pub id: String,
    /// Human-readable model name
    pub name: Option<String>,
    /// Whether the model is free to use
    pub is_free: Option<bool>,
    /// Maximum context window size in tokens
    pub context_window: Option<u32>,
}

/// List available AI models from a specific provider.
///
/// # Arguments
///
/// * `provider` - Provider name (e.g., "openrouter", "openai")
///
/// # Returns
///
/// A `ModelsResult` containing the list of models
pub async fn run_list(provider: &str) -> anyhow::Result<ModelsResult> {
    let token_provider = CliTokenProvider;
    let models = aptu_core::list_models(&token_provider, provider).await?;

    let serializable_models: Vec<SerializableModelInfo> = models
        .into_iter()
        .map(|m| SerializableModelInfo {
            id: m.id,
            name: m.name,
            is_free: m.is_free,
            context_window: m.context_window,
        })
        .collect();

    Ok(ModelsResult {
        provider: provider.to_string(),
        models: serializable_models,
    })
}

/// List available AI models from all available providers.
///
/// # Returns
///
/// A `ModelsResultMulti` containing results from all providers
pub async fn run_list_all() -> anyhow::Result<ModelsResultMulti> {
    let token_provider = CliTokenProvider;
    let providers = aptu_core::ai::registry::all_providers();

    let mut results = Vec::new();

    for provider_config in providers {
        match aptu_core::list_models(&token_provider, provider_config.name).await {
            Ok(models) => {
                let serializable_models: Vec<SerializableModelInfo> = models
                    .into_iter()
                    .map(|m| SerializableModelInfo {
                        id: m.id,
                        name: m.name,
                        is_free: m.is_free,
                        context_window: m.context_window,
                    })
                    .collect();

                results.push(ModelsResult {
                    provider: provider_config.name.to_string(),
                    models: serializable_models,
                });
            }
            Err(e) => {
                warn!(
                    "Failed to fetch models from {}: {}",
                    provider_config.name, e
                );
                // Continue with other providers
            }
        }
    }

    Ok(ModelsResultMulti { results })
}
