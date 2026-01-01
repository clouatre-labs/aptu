// SPDX-License-Identifier: Apache-2.0

//! Models command handler.

use anyhow::Result;
use aptu_core::ai::registry::get_provider;

use crate::commands::types::ModelsResult;

const DEFAULT_PROVIDER: &str = "openrouter";

/// List available models from a provider.
///
/// # Arguments
///
/// * `provider` - Optional provider name. If not provided, uses default provider.
/// * `free_only` - Filter to free tier models only.
///
/// # Returns
///
/// `ModelsResult` with list of models and provider name.
#[allow(clippy::unused_async)]
pub async fn run_list(provider: Option<String>, free_only: bool) -> Result<ModelsResult> {
    use crate::commands::types::SerializableModelInfo;

    // Determine provider to use
    let provider_name = match provider {
        Some(name) => name,
        None => {
            // Default to first provider
            aptu_core::ai::all_providers()
                .first()
                .map_or_else(|| DEFAULT_PROVIDER.to_string(), |p| p.name.to_string())
        }
    };

    // Validate provider exists
    if get_provider(&provider_name).is_none() {
        anyhow::bail!("Unknown provider: {provider_name}");
    }

    // Fetch models from static registry
    let models = aptu_core::list_models(&provider_name).await?;

    // Filter to free models if requested
    let mut filtered_models = models;
    if free_only {
        filtered_models.retain(|m| m.is_free);
    }

    // Convert to serializable_models format
    let serializable_models: Vec<SerializableModelInfo> = filtered_models
        .into_iter()
        .map(SerializableModelInfo::from)
        .collect();

    Ok(ModelsResult {
        models: serializable_models,
        provider: provider_name,
    })
}
