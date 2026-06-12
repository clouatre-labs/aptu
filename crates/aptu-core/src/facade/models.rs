// SPDX-License-Identifier: Apache-2.0

//! Model listing and validation facade functions.

use tracing::instrument;

use crate::auth::TokenProvider;
use crate::error::AptuError;

/// Lists available models from a provider API with caching.
///
/// This function fetches the list of available models from a provider's API,
/// with automatic caching and TTL validation. If the cache is valid, it returns
/// cached data. Otherwise, it fetches from the API and updates the cache.
///
/// # Arguments
///
/// * `provider` - Token provider for API credentials
/// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
///
/// # Returns
///
/// A vector of `ModelInfo` structs with available models.
///
/// # Errors
///
/// Returns an error if:
/// - Provider is not found
/// - API request fails
/// - Response parsing fails
#[instrument(skip(provider), fields(provider_name))]
pub async fn list_models(
    provider: &dyn TokenProvider,
    provider_name: &str,
) -> crate::Result<Vec<crate::ai::registry::CachedModel>> {
    use crate::ai::registry::{CachedModelRegistry, ModelRegistry};
    use crate::cache::cache_dir;

    let cache_dir = cache_dir();
    let registry =
        CachedModelRegistry::new(cache_dir, crate::cache::DEFAULT_MODEL_TTL_SECS, provider);

    registry
        .list_models(provider_name)
        .await
        .map_err(|e| AptuError::ModelRegistry {
            message: format!("Failed to list models: {e}"),
        })
}

/// Validates if a model exists for a provider.
///
/// This function checks if a specific model identifier is available for a provider,
/// using the cached model registry with automatic caching.
///
/// # Arguments
///
/// * `provider` - Token provider for API credentials
/// * `provider_name` - Name of the provider (e.g., "openrouter", "gemini")
/// * `model_id` - Model identifier to validate
///
/// # Returns
///
/// `true` if the model exists, `false` otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Provider is not found
/// - API request fails
/// - Response parsing fails
#[instrument(skip(provider), fields(provider_name, model_id))]
pub async fn validate_model(
    provider: &dyn TokenProvider,
    provider_name: &str,
    model_id: &str,
) -> crate::Result<bool> {
    use crate::ai::registry::{CachedModelRegistry, ModelRegistry};
    use crate::cache::cache_dir;

    let cache_dir = cache_dir();
    let registry =
        CachedModelRegistry::new(cache_dir, crate::cache::DEFAULT_MODEL_TTL_SECS, provider);

    registry
        .model_exists(provider_name, model_id)
        .await
        .map_err(|e| AptuError::ModelRegistry {
            message: format!("Failed to validate model: {e}"),
        })
}
