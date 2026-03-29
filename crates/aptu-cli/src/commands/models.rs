// SPDX-License-Identifier: Apache-2.0

//! Models command handler for listing AI models from providers.

use crate::cli::SortBy;
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
    /// Provider name this model belongs to
    pub provider: String,
}

/// Filter models by minimum context window size.
///
/// # Arguments
///
/// * `models` - Vector of models to filter
/// * `min_context` - Minimum context window size in tokens (optional)
///
/// # Returns
///
/// Filtered vector of models
fn filter_by_min_context(
    models: Vec<SerializableModelInfo>,
    min_context: Option<u32>,
) -> Vec<SerializableModelInfo> {
    match min_context {
        None => models,
        Some(min) => models
            .into_iter()
            .filter(|m| m.context_window.is_some_and(|ctx| ctx >= min))
            .collect(),
    }
}

/// Filter models by name substring (case-insensitive).
///
/// # Arguments
///
/// * `models` - Vector of models to filter
/// * `filter` - Case-insensitive substring to match against id and name (optional)
///
/// # Returns
///
/// Filtered vector of models
fn filter_by_name(
    models: Vec<SerializableModelInfo>,
    filter: Option<&str>,
) -> Vec<SerializableModelInfo> {
    match filter {
        None => models,
        Some(pat) => {
            let pat_lower = pat.to_lowercase();
            models
                .into_iter()
                .filter(|m| {
                    m.id.to_lowercase().contains(&pat_lower)
                        || m.name
                            .as_deref()
                            .is_some_and(|n| n.to_lowercase().contains(&pat_lower))
                })
                .collect()
        }
    }
}

/// Sort models by the specified field.
///
/// # Arguments
///
/// * `models` - Vector of models to sort
/// * `sort_by` - Field to sort by (Name or Context)
///
/// # Returns
///
/// Sorted vector of models
fn sort_models(
    mut models: Vec<SerializableModelInfo>,
    sort_by: SortBy,
) -> Vec<SerializableModelInfo> {
    match sort_by {
        SortBy::Name => {
            models.sort_by(|a, b| {
                let a_name = a.name.as_deref().unwrap_or(&a.id).to_lowercase();
                let b_name = b.name.as_deref().unwrap_or(&b.id).to_lowercase();
                a_name.cmp(&b_name)
            });
        }
        SortBy::Context => {
            models.sort_by(|a, b| {
                match (a.context_window, b.context_window) {
                    (Some(a_ctx), Some(b_ctx)) => b_ctx.cmp(&a_ctx), // Descending
                    (Some(_), None) => std::cmp::Ordering::Less,     // Models with context first
                    (None, Some(_)) => std::cmp::Ordering::Greater,  // Models without context last
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
    }
    models
}

/// List available AI models from a specific provider.
///
/// # Arguments
///
/// * `provider` - Provider name (e.g., "openrouter", "openai")
/// * `sort_by` - Field to sort by (Name or Context)
/// * `min_context` - Minimum context window size in tokens (optional)
/// * `filter` - Case-insensitive substring filter on id or name (optional)
///
/// # Returns
///
/// A `ModelsResult` containing the list of models
pub async fn run_list(
    provider: &str,
    sort_by: SortBy,
    min_context: Option<u32>,
    filter: Option<&str>,
) -> anyhow::Result<ModelsResult> {
    let token_provider = CliTokenProvider;
    let models = aptu_core::list_models(&token_provider, provider).await?;

    let mut serializable_models: Vec<SerializableModelInfo> = models
        .into_iter()
        .map(|m| SerializableModelInfo {
            id: m.id,
            name: m.name,
            is_free: m.is_free,
            context_window: m.context_window,
            provider: m.provider,
        })
        .collect();

    // Apply filtering first, then sorting
    serializable_models = filter_by_min_context(serializable_models, min_context);
    serializable_models = filter_by_name(serializable_models, filter);
    serializable_models = sort_models(serializable_models, sort_by);

    Ok(ModelsResult {
        provider: provider.to_string(),
        models: serializable_models,
    })
}

/// List available AI models from all available providers.
///
/// # Arguments
///
/// * `filter` - Case-insensitive substring filter on id or name (optional)
///
/// # Returns
///
/// A `ModelsResultMulti` containing results from all providers
pub async fn run_list_all(filter: Option<&str>) -> anyhow::Result<ModelsResultMulti> {
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
                        provider: m.provider,
                    })
                    .collect();

                let serializable_models = filter_by_name(serializable_models, filter);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model(
        id: &str,
        name: Option<&str>,
        context_window: Option<u32>,
    ) -> SerializableModelInfo {
        SerializableModelInfo {
            id: id.to_string(),
            name: name.map(String::from),
            is_free: None,
            context_window,
            provider: "test".to_string(),
        }
    }

    // --- filter_by_min_context ---

    #[test]
    fn test_filter_by_min_context_empty_list() {
        let result = filter_by_min_context(vec![], Some(8000));
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_by_min_context_none_filter() {
        let models = vec![
            make_model("model1", Some("Model 1"), Some(4000)),
            make_model("model2", Some("Model 2"), Some(8000)),
        ];
        let result = filter_by_min_context(models, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_min_context_with_threshold() {
        let models = vec![
            make_model("model1", Some("Model 1"), Some(4000)),
            make_model("model2", Some("Model 2"), Some(8000)),
            make_model("model3", Some("Model 3"), Some(16000)),
        ];
        let result = filter_by_min_context(models, Some(8000));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "model2");
        assert_eq!(result[1].id, "model3");
    }

    #[test]
    fn test_filter_by_min_context_excludes_none_values() {
        let models = vec![
            make_model("model1", Some("Model 1"), None),
            make_model("model2", Some("Model 2"), Some(8000)),
        ];
        let result = filter_by_min_context(models, Some(4000));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "model2");
    }

    // --- filter_by_name ---

    #[test]
    fn test_filter_by_name_match() {
        // Arrange
        let models = vec![
            make_model("gemini-pro", Some("Gemini Pro"), Some(128000)),
            make_model("gpt-4o", Some("GPT-4o"), Some(128000)),
        ];
        // Act
        let result = filter_by_name(models, Some("gemini"));
        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "gemini-pro");
    }

    #[test]
    fn test_filter_by_name_none_returns_all() {
        // Arrange
        let models = vec![
            make_model("gemini-pro", Some("Gemini Pro"), None),
            make_model("gpt-4o", Some("GPT-4o"), None),
        ];
        // Act
        let result = filter_by_name(models, None);
        // Assert
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_name_case_insensitive() {
        // Arrange
        let models = vec![
            make_model("gemini-pro", Some("Gemini Pro"), None),
            make_model("gpt-4o", Some("GPT-4o"), None),
        ];
        // Act
        let result = filter_by_name(models, Some("GEMINI"));
        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "gemini-pro");
    }

    // --- sort_models ---

    #[test]
    fn test_sort_models_by_name() {
        let models = vec![
            make_model("model_c", Some("Zebra Model"), Some(4000)),
            make_model("model_a", Some("Alpha Model"), Some(8000)),
            make_model("model_b", Some("Beta Model"), Some(16000)),
        ];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result[0].id, "model_a");
        assert_eq!(result[1].id, "model_b");
        assert_eq!(result[2].id, "model_c");
    }

    #[test]
    fn test_sort_models_by_name_case_insensitive() {
        let models = vec![
            make_model("model1", Some("zebra"), Some(4000)),
            make_model("model2", Some("ALPHA"), Some(8000)),
        ];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result[0].id, "model2");
        assert_eq!(result[1].id, "model1");
    }

    #[test]
    fn test_sort_models_by_context_descending() {
        let models = vec![
            make_model("model1", Some("Model 1"), Some(4000)),
            make_model("model2", Some("Model 2"), Some(8000)),
            make_model("model3", Some("Model 3"), Some(16000)),
        ];
        let result = sort_models(models, SortBy::Context);
        assert_eq!(result[0].id, "model3");
        assert_eq!(result[1].id, "model2");
        assert_eq!(result[2].id, "model1");
    }

    #[test]
    fn test_sort_models_by_context_none_values_last() {
        let models = vec![
            make_model("model1", Some("Model 1"), None),
            make_model("model2", Some("Model 2"), Some(8000)),
            make_model("model3", Some("Model 3"), None),
        ];
        let result = sort_models(models, SortBy::Context);
        assert_eq!(result[0].id, "model2");
    }

    #[test]
    fn test_sort_models_single_item() {
        let models = vec![make_model("model1", Some("Model 1"), Some(4000))];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "model1");
    }

    #[test]
    fn test_filter_and_sort_combined() {
        let models = vec![
            make_model("model1", Some("Zebra"), Some(4000)),
            make_model("model2", Some("Alpha"), Some(8000)),
            make_model("model3", Some("Beta"), Some(16000)),
        ];
        let filtered = filter_by_min_context(models, Some(8000));
        let result = sort_models(filtered, SortBy::Name);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "model2");
        assert_eq!(result[1].id, "model3");
    }
}
