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
///
/// # Returns
///
/// A `ModelsResult` containing the list of models
pub async fn run_list(
    provider: &str,
    sort_by: SortBy,
    min_context: Option<u32>,
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
        })
        .collect();

    // Apply filtering first, then sorting
    serializable_models = filter_by_min_context(serializable_models, min_context);
    serializable_models = sort_models(serializable_models, sort_by);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_by_min_context_empty_list() {
        let models = vec![];
        let result = filter_by_min_context(models, Some(8000));
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_by_min_context_none_filter() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Model 1".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
        ];
        let result = filter_by_min_context(models, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_min_context_with_threshold() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Model 1".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
            SerializableModelInfo {
                id: "model3".to_string(),
                name: Some("Model 3".to_string()),
                is_free: Some(true),
                context_window: Some(16000),
            },
        ];
        let result = filter_by_min_context(models, Some(8000));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "model2");
        assert_eq!(result[1].id, "model3");
    }

    #[test]
    fn test_filter_by_min_context_excludes_none_values() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Model 1".to_string()),
                is_free: Some(true),
                context_window: None,
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
        ];
        let result = filter_by_min_context(models, Some(4000));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "model2");
    }

    #[test]
    fn test_sort_models_by_name() {
        let models = vec![
            SerializableModelInfo {
                id: "model_c".to_string(),
                name: Some("Zebra Model".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model_a".to_string(),
                name: Some("Alpha Model".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
            SerializableModelInfo {
                id: "model_b".to_string(),
                name: Some("Beta Model".to_string()),
                is_free: Some(true),
                context_window: Some(16000),
            },
        ];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result[0].id, "model_a");
        assert_eq!(result[1].id, "model_b");
        assert_eq!(result[2].id, "model_c");
    }

    #[test]
    fn test_sort_models_by_name_case_insensitive() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("zebra".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("ALPHA".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
        ];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result[0].id, "model2");
        assert_eq!(result[1].id, "model1");
    }

    #[test]
    fn test_sort_models_by_context_descending() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Model 1".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
            SerializableModelInfo {
                id: "model3".to_string(),
                name: Some("Model 3".to_string()),
                is_free: Some(true),
                context_window: Some(16000),
            },
        ];
        let result = sort_models(models, SortBy::Context);
        assert_eq!(result[0].id, "model3"); // 16000
        assert_eq!(result[1].id, "model2"); // 8000
        assert_eq!(result[2].id, "model1"); // 4000
    }

    #[test]
    fn test_sort_models_by_context_none_values_last() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Model 1".to_string()),
                is_free: Some(true),
                context_window: None,
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Model 2".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
            SerializableModelInfo {
                id: "model3".to_string(),
                name: Some("Model 3".to_string()),
                is_free: Some(true),
                context_window: None,
            },
        ];
        let result = sort_models(models, SortBy::Context);
        assert_eq!(result[0].id, "model2"); // Has context window
        assert_eq!(result[1].id, "model1"); // None
        assert_eq!(result[2].id, "model3"); // None
    }

    #[test]
    fn test_sort_models_single_item() {
        let models = vec![SerializableModelInfo {
            id: "model1".to_string(),
            name: Some("Model 1".to_string()),
            is_free: Some(true),
            context_window: Some(4000),
        }];
        let result = sort_models(models, SortBy::Name);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "model1");
    }

    #[test]
    fn test_filter_and_sort_combined() {
        let models = vec![
            SerializableModelInfo {
                id: "model1".to_string(),
                name: Some("Zebra".to_string()),
                is_free: Some(true),
                context_window: Some(4000),
            },
            SerializableModelInfo {
                id: "model2".to_string(),
                name: Some("Alpha".to_string()),
                is_free: Some(false),
                context_window: Some(8000),
            },
            SerializableModelInfo {
                id: "model3".to_string(),
                name: Some("Beta".to_string()),
                is_free: Some(true),
                context_window: Some(16000),
            },
        ];
        // Filter for min_context >= 8000, then sort by name
        let filtered = filter_by_min_context(models, Some(8000));
        let result = sort_models(filtered, SortBy::Name);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "model2"); // Alpha
        assert_eq!(result[1].id, "model3"); // Beta
    }
}
