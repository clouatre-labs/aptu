// SPDX-License-Identifier: Apache-2.0

//! Typed-judge (`TypeSafe` Jev) configuration.

use serde::{Deserialize, Serialize};

/// Typed-judge configuration.
///
/// Controls the opt-in `typesafe_judge` integration. Disabled by default;
/// absent `[judge]` sections deserialize to defaults with no warnings.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct JudgeConfig {
    /// Whether typed-judge calls are enabled (default: `false`).
    pub enabled: bool,
    /// Optional API base URL (default: the built-in `TypeSafe` Jev endpoint).
    pub api_base: Option<String>,
    /// Optional judge model override passed through to the API.
    pub model: Option<String>,
}

impl JudgeConfig {
    /// Validate internal consistency of judge configuration.
    ///
    /// Returns a list of warning strings for any misconfigured values.
    /// The caller should emit these warnings via `tracing::warn!` or similar.
    #[must_use]
    pub fn validate_consistency(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.enabled && self.api_base.is_none() {
            warnings.push(
                "[judge] enabled = true with no api_base set: the built-in default endpoint will be used"
                    .to_string(),
            );
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::items_after_test_module)]
    use super::*;

    #[test]
    fn test_judge_config_defaults_disabled() {
        let config = JudgeConfig::default();
        assert!(!config.enabled, "judge should be disabled by default");
        assert!(config.api_base.is_none());
        assert!(config.model.is_none());
    }

    #[test]
    fn test_validate_consistency_enabled_without_api_base_warns() {
        let config = JudgeConfig {
            enabled: true,
            ..JudgeConfig::default()
        };
        let warnings = config.validate_consistency();
        assert_eq!(warnings.len(), 1, "should produce exactly 1 warning");
    }

    #[test]
    fn test_absent_judge_section_deserializes_to_default() {
        #[derive(Debug, Default, Deserialize)]
        #[serde(default)]
        struct Wrapper {
            judge: JudgeConfig,
        }
        let wrapper: Wrapper = toml::from_str("").expect("empty config deserializes");
        assert!(!wrapper.judge.enabled);
        assert!(wrapper.judge.api_base.is_none());
    }
}
