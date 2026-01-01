// SPDX-License-Identifier: Apache-2.0

//! Models command handler for listing AI models from providers.

use crate::provider::CliTokenProvider;
use serde::{Deserialize, Serialize};

/// Result of listing models from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResult {
    /// Provider name
    pub provider: String,
    /// List of models
    pub models: Vec<SerializableModelInfo>,
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

/// List available AI models from a provider.
///
/// # Arguments
///
/// * `provider` - Provider name (e.g., "openrouter", "openai")
/// * `free_only` - If true, filter to only free models
///
/// # Returns
///
/// A `ModelsResult` containing the list of models
pub async fn run_list(provider: &str, free_only: bool) -> anyhow::Result<ModelsResult> {
    let token_provider = CliTokenProvider;
    let models = aptu_core::list_models(&token_provider, provider).await?;

    let mut serializable_models: Vec<SerializableModelInfo> = models
        .into_iter()
        .map(|m| SerializableModelInfo {
            id: m.id,
            name: m.name,
            is_free: m.is_free,
            context_window: m.context_window,
        })
        .collect();

    // Filter to free models if requested
    if free_only {
        serializable_models.retain(|m| m.is_free.unwrap_or(false));
    }

    Ok(ModelsResult {
        provider: provider.to_string(),
        models: serializable_models,
    })
}
